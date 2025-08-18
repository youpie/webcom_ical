-- Add up migration script here
CREATE TABLE user_properties (
    id SERIAL PRIMARY KEY,
    send_mail_new_shift BOOLEAN NOT NULL,
    send_mail_updated_shift BOOLEAN NOT NULL,
    send_mail_removed_shift BOOLEAN NOT NULL,
    send_failed_signin_mail BOOLEAN NOT NULL,
    send_welcome_mail BOOLEAN NOT NULL,
    send_error_mail BOOLEAN NOT NULL,
    split_night_shift BOOLEAN NOT NULL,
    stop_midnight_shift BOOLEAN NOT NULL
);