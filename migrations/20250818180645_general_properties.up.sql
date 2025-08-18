-- Add up migration script here
CREATE TABLE general_properties (
    id SERIAL PRIMARY KEY,
    save_target TEXT NOT NULL,
    ical_domain TEXT NOT NULL,
    webcal_domain TEXT NOT NULL,
    pdf_shift_domain TEXT NOT NULL,
    signin_fail_execution_reduce INT NOT NULL,
    signin_fail_mail_reduce INT NOT NULL,
    execution_interval_minutes INT NOT NULL,
    expected_execution_time_seconds INT NOT NULL,
    execution_retry_count BIGINT NOT NULL,
    email_properties_id BIGINT NOT NULL REFERENCES email_properties(id),
    kuma_properties_id BIGINT NOT NULL REFERENCES kuma_properties(id),
    donation_text_id BIGINT NOT NULL REFERENCES donation_text(id)
);