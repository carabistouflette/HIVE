use uuid::Uuid;

// Helper function to generate new UUID strings
pub fn generate_id() -> String {
    Uuid::new_v4().to_string()
}