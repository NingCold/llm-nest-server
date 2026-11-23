\set ON_ERROR_STOP on
BEGIN;
SELECT pg_advisory_xact_lock(731921);
-- Consume one pre-upgrade entitlement and pin the independently verified device.
WITH candidate AS (
    SELECT d.account_id,d.device_id FROM cloud_crypto_devices d JOIN accounts a ON a.id=d.account_id
    WHERE d.account_id=:'account'::uuid AND d.device_id=:'device' AND NOT d.revoked
      AND a.kind='human' AND a.status='active' AND d.node_id IS NULL
      AND d.device_keys->'keys'->>('ed25519:'||d.device_id)=:'fingerprint'
      AND NOT EXISTS(SELECT FROM cloud_crypto_devices seen WHERE seen.account_id=d.account_id AND seen.authorized)
), consumed AS (
    DELETE FROM cloud_crypto_legacy_enrollment WHERE account_id IN (SELECT account_id FROM candidate)
    RETURNING account_id
)
UPDATE cloud_crypto_devices d SET authorized=true FROM candidate c JOIN consumed USING(account_id)
WHERE d.account_id=c.account_id AND d.device_id=c.device_id;
\if :ROW_COUNT
COMMIT;
\echo Legacy enrollment completed.
\else
ROLLBACK;
\echo No device enrolled. Check the three identifiers and the one-time legacy entitlement.
\quit 1
\endif
