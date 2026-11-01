-- A transactional singleton serializes public directory revisions, including change-back.
CREATE TABLE cloud_crypto_device_directory (
 singleton boolean PRIMARY KEY DEFAULT true CHECK (singleton),
 revision bigint NOT NULL DEFAULT 1
);
INSERT INTO cloud_crypto_device_directory(singleton) VALUES(true);

CREATE FUNCTION cloud_crypto_directory_changed() RETURNS trigger LANGUAGE plpgsql AS $$
BEGIN
 UPDATE cloud_crypto_device_directory SET revision=revision+1 WHERE singleton;
 RETURN NULL;
END;
$$;

CREATE TRIGGER cloud_crypto_devices_directory_insert_delete
AFTER INSERT OR DELETE ON cloud_crypto_devices
FOR EACH ROW EXECUTE FUNCTION cloud_crypto_directory_changed();
CREATE TRIGGER cloud_crypto_devices_directory_update
AFTER UPDATE ON cloud_crypto_devices
FOR EACH ROW WHEN (ROW(OLD.account_id, OLD.device_id, OLD.device_keys, OLD.revoked, OLD.node_id, OLD.binding_version) IS DISTINCT FROM ROW(NEW.account_id, NEW.device_id, NEW.device_keys, NEW.revoked, NEW.node_id, NEW.binding_version))
EXECUTE FUNCTION cloud_crypto_directory_changed();

CREATE TRIGGER cloud_crypto_signing_keys_directory_insert_delete
AFTER INSERT OR DELETE ON cloud_crypto_signing_keys
FOR EACH ROW EXECUTE FUNCTION cloud_crypto_directory_changed();
CREATE TRIGGER cloud_crypto_signing_keys_directory_update
AFTER UPDATE ON cloud_crypto_signing_keys
FOR EACH ROW WHEN (ROW(OLD.account_id, OLD.master_key, OLD.self_signing_key, OLD.user_signing_key) IS DISTINCT FROM ROW(NEW.account_id, NEW.master_key, NEW.self_signing_key, NEW.user_signing_key))
EXECUTE FUNCTION cloud_crypto_directory_changed();

CREATE TRIGGER cloud_ai_bindings_directory_insert_delete
AFTER INSERT OR DELETE ON cloud_ai_bindings
FOR EACH ROW EXECUTE FUNCTION cloud_crypto_directory_changed();
CREATE TRIGGER cloud_ai_bindings_directory_update
AFTER UPDATE ON cloud_ai_bindings
FOR EACH ROW WHEN (ROW(OLD.ai_id, OLD.node_id, OLD.version) IS DISTINCT FROM ROW(NEW.ai_id, NEW.node_id, NEW.version))
EXECUTE FUNCTION cloud_crypto_directory_changed();

CREATE TRIGGER accounts_directory_insert_delete
AFTER INSERT OR DELETE ON accounts
FOR EACH ROW EXECUTE FUNCTION cloud_crypto_directory_changed();
CREATE TRIGGER accounts_directory_update
AFTER UPDATE ON accounts
FOR EACH ROW WHEN (ROW(OLD.id, OLD.status) IS DISTINCT FROM ROW(NEW.id, NEW.status))
EXECUTE FUNCTION cloud_crypto_directory_changed();
