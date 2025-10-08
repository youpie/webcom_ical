
use entity::prelude::{DonationText, EmailProperties, GeneralPropertiesDb, KumaProperties};
use sea_orm::{Database, EntityTrait};

use crate::variables::GeneralProperties;

pub async fn get_kuma_email() {
    let db = Database::connect("postgres://postgres:123qwerty@localhost/postgres").await.unwrap();
    let properties: GeneralProperties = GeneralPropertiesDb::find_by_id(1).left_join(KumaProperties).left_join(EmailProperties).left_join(DonationText).into_partial_model().one(&db).await.unwrap().unwrap();
    println!("{properties:#?}");

}