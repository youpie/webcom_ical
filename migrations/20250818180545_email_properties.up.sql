-- Add up migration script here
CREATE TABLE email_properties (
    id SERIAL PRIMARY KEY,
    smtp_server TEXT NOT NULL,
    smtp_username TEXT NOT NULL,
    smtp_password TEXT NOT NULL,
    mail_from TEXT NOT NULL,
    support_mail TEXT NOT NULL
);