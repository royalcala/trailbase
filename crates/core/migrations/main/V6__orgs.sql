-- Organization control plane shared by main and org databases.

CREATE TABLE _org (
  id        BLOB PRIMARY KEY NOT NULL CHECK(is_uuid_v7(id)) DEFAULT (uuid_v7()),
  name      TEXT NOT NULL,
  slug      TEXT NOT NULL CHECK(length(slug) > 0),

  created   INTEGER DEFAULT (UNIXEPOCH()) NOT NULL,
  updated   INTEGER DEFAULT (UNIXEPOCH()) NOT NULL
) STRICT;

CREATE UNIQUE INDEX __org__slug_index ON _org (slug);

CREATE TRIGGER __org__updated_trigger AFTER UPDATE ON _org FOR EACH ROW
  BEGIN
    UPDATE _org SET updated = UNIXEPOCH() WHERE id = OLD.id;
  END;

CREATE TABLE _org_membership (
  org       BLOB NOT NULL REFERENCES _org(id) ON DELETE CASCADE,
  user      BLOB NOT NULL REFERENCES _user(id) ON DELETE CASCADE,
  role      TEXT NOT NULL DEFAULT 'member',

  created   INTEGER DEFAULT (UNIXEPOCH()) NOT NULL,
  updated   INTEGER DEFAULT (UNIXEPOCH()) NOT NULL,

  PRIMARY KEY (org, user)
) STRICT;

CREATE INDEX __org_membership__user_index ON _org_membership (user);
CREATE INDEX __org_membership__org_index ON _org_membership (org);

CREATE TRIGGER __org_membership__updated_trigger AFTER UPDATE ON _org_membership FOR EACH ROW
  BEGIN
    UPDATE _org_membership SET updated = UNIXEPOCH() WHERE org = OLD.org AND user = OLD.user;
  END;

CREATE TABLE _org_invitation (
  id        BLOB PRIMARY KEY NOT NULL CHECK(is_uuid_v7(id)) DEFAULT (uuid_v7()),
  org       BLOB NOT NULL REFERENCES _org(id) ON DELETE CASCADE,
  email     TEXT NOT NULL CHECK(is_email(email)),
  invited_by BLOB NOT NULL REFERENCES _user(id) ON DELETE CASCADE,
  role      TEXT NOT NULL DEFAULT 'member',
  token     TEXT NOT NULL,
  accepted  INTEGER DEFAULT FALSE NOT NULL,

  created   INTEGER DEFAULT (UNIXEPOCH()) NOT NULL,
  expires   INTEGER NOT NULL
) STRICT;

CREATE UNIQUE INDEX __org_invitation__token_index ON _org_invitation (token);
CREATE INDEX __org_invitation__org_index ON _org_invitation (org);
CREATE INDEX __org_invitation__email_index ON _org_invitation (email);
