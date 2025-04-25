-- sqlfluff:dialect:sqlite

ALTER TABLE urls
ADD COLUMN last_modified INTEGER;

CREATE TRIGGER set_last_modified_on_insert
AFTER INSERT ON urls
FOR EACH ROW
WHEN new.last_modified IS NULL
BEGIN
UPDATE urls SET last_modified = unixepoch()
WHERE rowid = new.rowid;
END;

CREATE TRIGGER set_last_modified_on_update
AFTER UPDATE ON urls
FOR EACH ROW
BEGIN
UPDATE urls SET last_modified = unixepoch()
WHERE rowid = new.rowid;
END;
