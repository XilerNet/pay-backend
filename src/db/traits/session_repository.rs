use uuid::Uuid;

pub trait SessionRepository
where
    Self: Clone,
{
    async fn get_session(&self, token: &str) -> Result<Uuid, sqlx::Error>;
}
