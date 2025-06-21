#[derive(Debug, Clone)]
pub struct Label {
    name: String,
}

impl Label {
    pub fn new(name: String) -> Self {
        Self {
            name,
        }
    }

    pub fn name(&self) -> &str {
        // remove the @ in the front
        &self.name[1..]        
    }

    pub fn to_string(&self) -> String {
        self.name.clone()
    }
}