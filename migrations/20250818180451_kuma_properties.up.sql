-- Add up migration script here
CREATE TABLE kuma_properties (
    id SERIAL PRIMARY KEY,
    domain TEXT NOT NULL,
    smtp_server TEXT NOT NULL,
    smtp_username TEXT NOT NULL,
    smtp_password TEXT NOT NULL,
    mail_from TEXT NOT NULL,
    mail_port BIGINT NOT NULL,
    use_ssl BOOLEAN NOT NULL,
    offline_mail_resend_hours BIGINT NOT NULL
);