-- Add up migration script here
CREATE TABLE donation_text (
    id SERIAL PRIMARY KEY,
    donate_link TEXT NOT NULL,
    donate_text TEXT NOT NULL,
    donate_service_name TEXT NOT NULL,
    iban TEXT NOT NULL,
    iban_name TEXT NOT NULL
);