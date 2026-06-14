use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Supplier {
    pub id:             i64,
    pub name:           String,
    pub contact_person: Option<String>,
    pub email:          Option<String>,
    pub phone:          Option<String>,
    pub address:        Option<String>,
    pub notes:          Option<String>,
    pub created_at:     String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NewSupplier {
    pub name:           String,
    pub contact_person: Option<String>,
    pub email:          Option<String>,
    pub phone:          Option<String>,
    pub address:        Option<String>,
    pub notes:          Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn supplier_optional_fields_are_none_by_default() {
        let s = Supplier {
            id:             1,
            name:           "Acme Ltd".to_string(),
            contact_person: None,
            email:          None,
            phone:          None,
            address:        None,
            notes:          None,
            created_at:     "2026-01-01T00:00:00Z".to_string(),
        };
        assert!(s.contact_person.is_none());
        assert!(s.email.is_none());
        assert_eq!(s.name, "Acme Ltd");
    }

    #[test]
    fn new_supplier_with_all_fields() {
        let ns = NewSupplier {
            name:           "Sony Electronics".to_string(),
            contact_person: Some("Jane Doe".to_string()),
            email:          Some("jane@sony.com".to_string()),
            phone:          Some("+1 555 0100".to_string()),
            address:        Some("123 Main St".to_string()),
            notes:          Some("Primary camera supplier".to_string()),
        };
        assert_eq!(ns.name, "Sony Electronics");
        assert_eq!(ns.contact_person.as_deref(), Some("Jane Doe"));
    }
}
