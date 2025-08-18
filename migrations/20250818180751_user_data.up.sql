-- Add up migration script here
CREATE TABLE user_data (
    id SERIAL PRIMARY KEY,
    personeelsnummer VARCHAR(100) NOT NULL,
    wachtwoord VARCHAR(100) NOT NULL,
    email VARCHAR(100) NOT NULL,
    file_name TEXT NOT NULL,
    properties_id BIGINT NOT NULL REFERENCES user_properties(id),
    custom_general_properties_id BIGINT REFERENCES general_properties(id)
);