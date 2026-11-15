use crate::layer::Layer;

impl Layer {
    pub fn exists(&self) -> bool {
        self.base.exists()
            || self.pid_path().exists()
            || self.lock_path().exists()
    }
}
