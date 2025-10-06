
use entity::prelude::{GeneralProperties, KumaProperties};
use sea_orm::{Database, DerivePartialModel, EntityTrait};

#[derive(Debug, DerivePartialModel)]
#[sea_orm(entity = "GeneralProperties")]
pub struct GeneralPropertiesD {
    save_target: String,
    webcal_domain: String,
    pdf_shift_domain: String,
    signin_fail_mail_reduce: i32,
    execution_interval_minutes: i32,
    expected_exectution_time_seconds: i32,
    execution_retry_count: i32,
    #[sea_orm(nested)]
    kuma_properties: <entity::prelude::KumaProperties as EntityTrait>::Model
}

pub async fn get_kuma_email() {
    let db = Database::connect("postgres://postgres:123qwerty@localhost/postgres").await.unwrap();
    let properties: GeneralPropertiesD = GeneralProperties::find_by_id(1).left_join(KumaProperties).into_partial_model().one(&db).await.unwrap().unwrap();
    println!("{properties:#?}");

}