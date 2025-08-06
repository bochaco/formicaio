-- Change default frequency to check latest version of node binary to six hours.
UPDATE settings SET node_bin_version_polling_freq_secs=60 * 60 * 6 WHERE node_bin_version_polling_freq_secs = 60 * 60 * 2;
