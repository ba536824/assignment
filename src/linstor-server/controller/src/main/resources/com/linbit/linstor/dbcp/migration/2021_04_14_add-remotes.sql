-- create tables for remotes
CREATE TABLE S3_REMOTES(
    UUID CHARACTER(36) NOT NULL,
    NAME VARCHAR(256) NOT NULL PRIMARY KEY,
    DSP_NAME VARCHAR(256) NOT NULL,
    FLAGS BIGINT NOT NULL,
    ENDPOINT VARCHAR(256) NOT NULL,
    BUCKET VARCHAR(63) NOT NULL,
    REGION VARCHAR(256) NOT NULL,
    ACCESS_KEY BLOB NOT NULL,
    SECRET_KEY BLOB NOT NULL
);

CREATE TABLE LINSTOR_REMOTES(
    UUID CHARACTER(36) NOT NULL,
    NAME VARCHAR(256) NOT NULL PRIMARY KEY,
    DSP_NAME VARCHAR(256) NOT NULL,
    FLAGS BIGINT NOT NULL,
    URL VARCHAR(2048) NOT NULL,
    ENCRYPTED_PASSPHRASE BLOB NULL,
    CLUSTER_ID CHARACTER(36) NULL
);