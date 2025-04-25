-- sqlfluff:dialect:sqlite

DROP TRIGGER set_last_modified_on_insert;
DROP TRIGGER set_last_modified_on_update;
ALTER TABLE urls DROP COLUMN last_modified;
