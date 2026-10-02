-- One-time switch from HTTP (0) to System (1) metrics mode: the current ant-node release
-- doesn't expose a metrics endpoint. Disabled (2) is left untouched.
UPDATE settings SET metrics_mode = 1 WHERE metrics_mode = 0;
