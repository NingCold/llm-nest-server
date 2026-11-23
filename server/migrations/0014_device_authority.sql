-- Existing human backup keys remain readable but must never receive a new session.
ALTER TABLE cloud_crypto_devices ADD COLUMN authorized boolean NOT NULL DEFAULT false;
ALTER TABLE cloud_crypto_backups ADD COLUMN sealed boolean NOT NULL DEFAULT false;
UPDATE cloud_crypto_backups b SET sealed=true FROM accounts a WHERE b.account_id=a.id AND a.kind='human';
ALTER TABLE cloud_crypto_recovery ADD COLUMN enrollment_key text;
CREATE TRIGGER cloud_crypto_device_authority_directory
AFTER UPDATE OF authorized ON cloud_crypto_devices
FOR EACH ROW WHEN (OLD.authorized IS DISTINCT FROM NEW.authorized)
EXECUTE FUNCTION cloud_crypto_directory_changed();
-- Only accounts with pre-upgrade encryption state can use the one-time operator enrollment.
CREATE TABLE cloud_crypto_legacy_enrollment(account_id uuid PRIMARY KEY REFERENCES accounts(id));
INSERT INTO cloud_crypto_legacy_enrollment
SELECT id FROM accounts a WHERE kind='human' AND (
  EXISTS (SELECT FROM cloud_crypto_signing_keys k WHERE k.account_id=a.id)
  OR EXISTS (SELECT FROM cloud_crypto_recovery r WHERE r.account_id=a.id)
  OR EXISTS (SELECT FROM cloud_crypto_backups b WHERE b.account_id=a.id));
