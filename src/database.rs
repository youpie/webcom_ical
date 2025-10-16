use entity::{
    donation_text, email_properties, general_properties_db, kuma_properties,
    prelude::{DonationText, EmailProperties, GeneralPropertiesDb, KumaProperties},
};
use migration::Alias;
use sea_orm::{Database, EntityTrait, QuerySelect, RelationTrait};

use crate::variables::{GeneralProperties, KumaProperties};

pub async fn get_kuma_email() {
    let db = Database::connect("postgres://postgres:123qwerty@localhost/postgres")
        .await
        .unwrap();

    let properties: GeneralProperties = GeneralPropertiesDb::find_by_id(1)
        .left_join(kuma_properties::Entity)
        .left_join(email_properties::Entity)
        .left_join(donation_text::Entity)
        .join_as(
            sea_orm::JoinType::LeftJoin,
            kuma_properties::Relation::EmailProperties.def(),
            "kuma_email",
        )
        .into_partial_model()
        .one(&db)
        .await
        .unwrap()
        .unwrap();
    println!("{properties:#?}");
}
