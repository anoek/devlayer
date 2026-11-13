VERSION := $(shell cargo metadata --no-deps --format-version 1 | jq -r '.packages[0].version')

###
### Development targets
###
all build:
	cargo build

# watch for changes and rebuild
watch:
	watchexec -w src -w Cargo.toml -- cargo build
	
build-debug-with-setuid: build
	sudo cp target/debug/devlayer target/debug/devlayer-setuid
	sudo chown root:root target/debug/devlayer-setuid
	sudo chmod u+s target/debug/devlayer-setuid
	
build-coverage-binary:	
	CARGO_INCREMENTAL=0 RUSTFLAGS='-Cinstrument-coverage' LLVM_PROFILE_FILE="coverage/profraw/cargo-test-%p-%m.profraw" cargo build --profile coverage --features coverage
	rm -f target/coverage/devlayer-setuid
	cp target/coverage/devlayer target/coverage/devlayer-setuid
	sudo chown root:root target/coverage/devlayer-setuid	
	sudo chmod u+s target/coverage/devlayer-setuid

audit:
	cargo audit
	
clippy lint:
	cargo clippy

clippy-fix:
	cargo clippy --fix --allow-dirty --allow-staged
	cargo fmt
	
install-pre-commit-hooks:
	@echo "### Installing pre-commit hooks..."
	echo "make ready-for-commit-tests" > .git/hooks/pre-commit
	chmod +x .git/hooks/pre-commit
	
ready-for-commit-tests: 
	@echo "### Formatting code"
	cargo fmt
	@echo "### Running pre-commit checks"
	cargo clippy
	@echo "### Checking for debug mode in tests"
	@DEBUG_MODE_COUNT=$$(grep "set_debug_mode(true)" tests/*.rs | wc -l); \
	if [ $$DEBUG_MODE_COUNT -gt 0 ]; then \
		echo "ERROR: Found $$DEBUG_MODE_COUNT instance(s) of set_debug_mode(true) in test files."; \
		echo "Debug mode should not be enabled in committed tests."; \
		grep "set_debug_mode(true)" tests/*.rs; \
		exit 1; \
	fi
	@if [ -f /usr/bin/npx ]; then \
		echo "### Checking spelling"; \
		npx cspell-cli --gitignore . \
	else \
		echo "### npx not found, skipping spelling check"; \
	fi


clean-bad-mounts:
	devlayer stop --all
	mount | grep 'overlay on ' | grep 'devlayer' | awk '{print $$3}' | xargs -I{} sudo umount {}



clean:
	sudo chown -R $(USER) .
	cargo clean
	rm -Rf --one-file-system coverage dist build generated-test-data
	sudo rm -f default*.profraw
	
clean-coverage-layers:
	devlayer stop 'devlayer-coverage-test-*'
	sudo rm --one-file-system -Rf ~/.devlayer/devlayer-coverage-test-*
	
###
### Testing	
###
	
test: build-coverage-binary
	# Note: This excludes tests marked with #[ignore] (like stop --all tests)
	sudo rm --one-file-system -Rf coverage
	mkdir -p coverage/profraw
	sudo mkdir -p /run/media/devlayer-coverage-testing
	RUST_BACKTRACE=1 CARGO_INCREMENTAL=0 RUSTFLAGS='-Cinstrument-coverage' LLVM_PROFILE_FILE="coverage/profraw/cargo-test-%p-%m.profraw" cargo test --profile coverage --features coverage -- --test-threads=1
	
coverage coverage-report: test update-coverage-report-and-show-json

update-coverage-report-and-show-json:
	make update-coverage-report
	cat coverage/html/coverage.json | jq

# We usually ignore the tests that conflict with external layers, namely `stop --all`. However 
test-with-stop-all: build-coverage-binary
	@echo "### Running all tests including --ignored tests"
	sudo rm --one-file-system -Rf coverage
	mkdir -p coverage/profraw
	RUST_BACKTRACE=1 CARGO_INCREMENTAL=0 RUSTFLAGS='-Cinstrument-coverage' LLVM_PROFILE_FILE="coverage/profraw/cargo-test-%p-%m.profraw" cargo test --profile coverage --features coverage -- --test-threads=1
	RUST_BACKTRACE=1 CARGO_INCREMENTAL=0 RUSTFLAGS='-Cinstrument-coverage' LLVM_PROFILE_FILE="coverage/profraw/cargo-test-%p-%m.profraw" cargo test --profile coverage --features coverage -- --test-threads=1 --ignored

coverage-with-stop-all: test-with-stop-all update-coverage-report-and-show-json

# NOTE: The full tests do their own coverage report generation in the vm-testing/80-generate-coverage-reports.sh script
update-coverage-report:	
	@BULMA_VERSION=1.0.2 grcov . --binary-path ./target/coverage/ -s . --llvm -t html --branch --ignore-not-existing --ignore "vendor/*" --ignore "tests/*" --ignore "src/config/structs.rs" --ignore "src/config/cli.rs" --ignore "target/*" -o coverage/
	@find coverage/html -type f -name "*.html" -exec sed -i 's/bulma@0.9.1/bulma@1.0.3/g' {} \;
	@find coverage/html -type f -name "*.html" -exec sed -i 's/<\/head>/<style> :root { --bulma-text: #c6c6c6; --bulma-white-l: 20%; --bulma-success-l: 23%; --bulma-success-90-l: 33%; --bulma-danger-l: 23%; --bulma-danger-s: 50%; --bulma-danger-90-l: 33%; } <\/style><\/head>/g' {} \;
	@echo "Coverage report generated at file://`pwd`/coverage/html/index.html"
	@echo "Coverage percentage: `jq '.message' coverage/html/coverage.json`"
	
	
# Perform only some named tests by doing something like `TEST=test_twisted make quick-test`
TEST ?= test_accept
quick-test: build-coverage-binary
	# test_accept
	RUST_BACKTRACE=1 CARGO_INCREMENTAL=0 RUSTFLAGS='-Cinstrument-coverage' LLVM_PROFILE_FILE="coverage/profraw/cargo-test-%p-%m.profraw" cargo test --profile coverage --features coverage -- --test-threads=1 \
		$(TEST)
	make update-coverage-report


coverage/html/coverage.json: update-coverage-report
	
coverage-check: coverage/html/coverage.json
	if jq -e '.message' coverage/html/coverage.json | grep -q 100; then \
		echo "Coverage is 100%"; \
	else \
		echo "Coverage is less than 100%"; \
		exit 1; \
	fi
	
# Full test requires that the host has been setup for vm testing. This can be
# done with the vm-testing/01-setup-host-for-vm-testing.sh script
full-test: prepared-coverage-vm
	cd vm-testing \
		&& ./30-build-image-and-start.sh \
		&& ./42-test-different-relative-paths.sh \
		&& ./45-test-different-filesystems.sh \
		&& ./50-start-package-testing-vms.sh \
		&& ./51-run-package-testing.sh \
		&& ./80-generate-coverage-reports.sh \
		&& ./99-cleanup.sh
	
prepared-coverage-vm: vm-testing/images/coverage-prepared.qcow2

vm-testing/images/coverage-prepared.qcow2: vm-testing/20-prepare-vm-images.sh
	cd vm-testing \
		&& ./20-prepare-vm-images.sh

install-cov-tools:
	#cargo install cargo-llvm-cov
	cargo install grcov

	
	

###
### Release targets
###

# Build all release binaries
build-release: 
	cargo build --release
	cargo build --release --target x86_64-unknown-linux-musl
	cargo build --release --target aarch64-unknown-linux-musl
	cargo build --release --target x86_64-unknown-linux-gnu
	cargo build --release --target aarch64-unknown-linux-gnu
	sudo bash -c ' \
		chown root:root target/release/devlayer && \
		chmod u+s target/release/devlayer && \
		chown root:root target/aarch64-unknown-linux-musl/release/devlayer && \
		chmod u+s target/aarch64-unknown-linux-musl/release/devlayer && \
		chown root:root target/x86_64-unknown-linux-musl/release/devlayer && \
		chmod u+s target/x86_64-unknown-linux-musl/release/devlayer && \
		chown root:root target/aarch64-unknown-linux-gnu/release/devlayer && \
		chmod u+s target/aarch64-unknown-linux-gnu/release/devlayer && \
		chown root:root target/x86_64-unknown-linux-gnu/release/devlayer && \
		chmod u+s target/x86_64-unknown-linux-gnu/release/devlayer'
	
build-for-profiling:
	cargo build --profile profiler

# Man page
build/devlayer.1: README.adoc Cargo.toml
	mkdir -p build
	cat README.adoc \
		| sed "s/:mansource:.*/:mansource: $(VERSION)/" \
		| asciidoctor -b manpage - -o build/devlayer.1
	
man build/devlayer.1.gz: build/devlayer.1
	gzip -9 < $< > build/devlayer.1.gz
	
# Shell completion scripts
completion-scripts: build
	mkdir -p build
	COMPLETE=bash ./target/debug/devlayer | sed 's|/.*/devlayer|devlayer|g' > build/devlayer-bash-completion
	COMPLETE=zsh ./target/debug/devlayer | sed 's|/.*/devlayer|devlayer|g' > build/devlayer-zsh-completion
	COMPLETE=fish ./target/debug/devlayer | sed 's|/.*/devlayer|devlayer|g' > build/devlayer-fish-completion
	
	
# Packaging
COMMON_FPM_FILES=\
	build/devlayer.1.gz=/usr/share/man/man1/devlayer.1.gz \
	build/devlayer-bash-completion=/usr/share/bash-completion/completions/devlayer \
	build/devlayer-fish-completion=/usr/share/fish/vendor_completions.d/devlayer.fish	

package: build/devlayer.1.gz build-release completion-scripts
	rm -Rf dist
	mkdir -p dist
	@echo "### Copying raw files to dist"
	tar -czvf dist/devlayer-static-$(VERSION)-x86_64.tar.gz target/x86_64-unknown-linux-musl/release/devlayer
	tar -czvf dist/devlayer-static-$(VERSION)-arm64.tar.gz target/aarch64-unknown-linux-musl/release/devlayer
	@echo "### Building static x86_64 targets"
	fpm \
		-t deb \
		--fpm-options-file packaging/common-fpm-options \
		--version $(VERSION) \
		--architecture x86_64 \
		--package dist/devlayer_$(VERSION)_amd64.deb \
		target/x86_64-unknown-linux-musl/release/devlayer=/usr/bin/devlayer \
		build/devlayer-zsh-completion=/usr/share/zsh/vendor-completions/_devlayer \
		$(COMMON_FPM_FILES)
	fpm \
		-t rpm \
		--fpm-options-file packaging/common-fpm-options \
		--version $(VERSION) \
		--architecture x86_64 \
		--package dist/devlayer.$(VERSION)-1.x86_64.rpm \
		target/x86_64-unknown-linux-musl/release/devlayer=/usr/bin/devlayer \
		build/devlayer-zsh-completion=/usr/share/zsh/site-functions/_devlayer \
		$(COMMON_FPM_FILES)
	fpm \
		-t pacman \
		--fpm-options-file packaging/common-fpm-options \
		--version $(VERSION) \
		--architecture x86_64 \
		--name devlayer-bin \
		--package dist/devlayer-bin-$(VERSION)-1-x86_64.pkg.tar.zst \
		target/x86_64-unknown-linux-musl/release/devlayer=/usr/bin/devlayer \
		build/devlayer-zsh-completion=/usr/share/zsh/site-functions/_devlayer \
		$(COMMON_FPM_FILES)
	@echo "### Building static arm64 targets"
	fpm \
		-t deb \
		--fpm-options-file packaging/common-fpm-options \
		--version $(VERSION) \
		--architecture arm64 \
		--package dist/devlayer_$(VERSION)_arm64.deb \
		target/aarch64-unknown-linux-musl/release/devlayer=/usr/bin/devlayer \
		$(COMMON_FPM_FILES)
	fpm \
		-t rpm \
		--fpm-options-file packaging/common-fpm-options \
		--version $(VERSION) \
		--architecture arm64 \
		--package dist/devlayer.$(VERSION)-1.arm64.rpm \
		target/aarch64-unknown-linux-musl/release/devlayer=/usr/bin/devlayer \
		$(COMMON_FPM_FILES)
	fpm \
		-t pacman \
		--fpm-options-file packaging/common-fpm-options \
		--version $(VERSION) \
		--architecture arm64 \
		--name devlayer-bin \
		--package dist/devlayer-bin-$(VERSION)-1-arm64.pkg.tar.zst \
		target/aarch64-unknown-linux-musl/release/devlayer=/usr/bin/devlayer \
		$(COMMON_FPM_FILES)



	
###
### Manual install / uninstall targets
###
	
install: build/devlayer.1.gz completion-scripts
	cargo build --release
	# Determine the installation directory based on the PATH
	@INSTALL_DIR=$$(if echo "$$PATH" | tr ':' '\n' | grep -q "^/usr/bin$$"; then \
		echo "/usr/bin"; \
	elif echo "$$PATH" | tr ':' '\n' | grep -q "^/bin$$"; then \
		echo "/bin"; \
	else \
		echo "/usr/bin"; \
	fi); \
	echo "Installing to $$INSTALL_DIR/devlayer"; \
	sudo cp target/release/devlayer "$$INSTALL_DIR/devlayer.new"; \
	sudo mv "$$INSTALL_DIR/devlayer.new" "$$INSTALL_DIR/devlayer"; \
	sudo chown root:root "$$INSTALL_DIR/devlayer"; \
	sudo chmod u+s "$$INSTALL_DIR/devlayer"
	sudo cp build/devlayer.1.gz /usr/share/man/man1/devlayer.1.gz
	sudo cp build/devlayer-bash-completion /usr/share/bash-completion/completions/devlayer
	sudo cp build/devlayer-zsh-completion /usr/share/zsh/site-functions/_devlayer
	sudo cp build/devlayer-fish-completion /usr/share/fish/vendor_completions.d/devlayer.fish
	
uninstall:
	sudo rm -f /usr/bin/devlayer \
	   /usr/share/man/man1/devlayer.1.gz \
	   /usr/share/bash-completion/completions/devlayer \
	   /usr/share/zsh/site-functions/_devlayer \
	   /usr/share/fish/vendor_completions.d/devlayer.fish
	
		


.PHONY: all build watch build-coverage-binary clippy lint clippy-fix \
	install-pre-commit-hooks ready-for-commit-tests clean clean-coverage-layers \
	test test-ignored quick-test coverage coverage-report update-coverage-report \
	full-coverage full-test update-coverage-report-and-show-json \
	full-test prepared-coverage-vm install install-cov-tools uninstall \
	build-release completion-scripts package man build-for-profiling
