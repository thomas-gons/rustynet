use indexmap::IndexMap;

pub struct HttpHeaders {
    headers: IndexMap<String, String>,
}

impl HttpHeaders {
    pub fn new() -> Self {
        Self {
            headers: IndexMap::new(),
        }
    }

    pub fn set_raw(&mut self, name: &str, value: &str) {
        self.headers.insert(name.to_string(), value.to_string());
    }

    pub fn get(&self, name: &str) -> Option<&String> {
        self.headers.get(name)
    }

    pub fn stringify(&self) -> String {
        let mut result = String::new();
        for (name, value) in &self.headers {
            result.push_str(&format!("{}: {}\r\n", name, value));
        }
        result
    }
}
