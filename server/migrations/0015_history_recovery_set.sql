ALTER TABLE cloud_history_migrations ADD COLUMN verification_recovery_digests text[];

-- During a rolling upgrade, a negative receipt must remain final for an old unsigned
-- control command. Older processes cannot create or complete a migration after this point.
CREATE FUNCTION cloud_require_history_control() RETURNS trigger LANGUAGE plpgsql AS $$
DECLARE expected text;
BEGIN
    IF TG_OP='INSERT' THEN expected:='begin:' || NEW.conversation;
    ELSE expected:='finish:' || NEW.id;
    END IF;
    IF current_setting('llmn.history_control', true) IS DISTINCT FROM expected THEN
        RAISE EXCEPTION 'verified device migration command required' USING ERRCODE='42501';
    END IF;
    RETURN NEW;
END;
$$;
CREATE TRIGGER cloud_history_signed_begin BEFORE INSERT ON cloud_history_migrations
FOR EACH ROW WHEN (NEW.state='active') EXECUTE FUNCTION cloud_require_history_control();
CREATE TRIGGER cloud_history_signed_finish BEFORE UPDATE OF state ON cloud_history_migrations
FOR EACH ROW WHEN (OLD.state IS DISTINCT FROM 'complete' AND NEW.state='complete')
EXECUTE FUNCTION cloud_require_history_control();
