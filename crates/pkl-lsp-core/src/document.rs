use std::collections::BTreeMap;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Document {
    pub uri: String,
    pub text: String,
    pub version: i32,
}

#[derive(Debug, Clone, Default)]
pub struct DocumentStore {
    documents: BTreeMap<String, Document>,
}

impl DocumentStore {
    pub fn open(&mut self, uri: impl Into<String>, text: impl Into<String>, version: i32) {
        let uri = uri.into();
        self.documents.insert(
            uri.clone(),
            Document {
                uri,
                text: text.into(),
                version,
            },
        );
    }

    pub fn change(&mut self, uri: impl Into<String>, text: impl Into<String>, version: i32) {
        self.open(uri, text, version);
    }

    pub fn close(&mut self, uri: &str) -> Option<Document> {
        self.documents.remove(uri)
    }

    pub fn get(&self, uri: &str) -> Option<&Document> {
        self.documents.get(uri)
    }

    pub fn iter(&self) -> impl Iterator<Item = &Document> {
        self.documents.values()
    }
}

#[cfg(test)]
mod tests {
    use super::DocumentStore;

    #[test]
    fn open_change_close_document() {
        let mut store = DocumentStore::default();
        store.open("file:///a.pkl", "name = \"a\"", 1);
        assert_eq!(store.get("file:///a.pkl").unwrap().version, 1);

        store.change("file:///a.pkl", "name = \"b\"", 2);
        assert_eq!(store.get("file:///a.pkl").unwrap().text, "name = \"b\"");

        assert!(store.close("file:///a.pkl").is_some());
        assert!(store.get("file:///a.pkl").is_none());
    }
}
