use entity::prelude::{GeneralProperties, KumaProperties};
use sea_orm::{ConnectOptions, Database, EntityTrait};

pub async fn get_kuma_email() {
    let db = Database::connect("postgres://postgres:123qwerty@localhost/postgres").await.unwrap();
    let properties = GeneralProperties::find_by_id(1).left_join(KumaProperties).one(&db).await.unwrap();

}