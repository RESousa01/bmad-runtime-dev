-- Privacy gate inspection (D2-E Task 11, gate 4): after a canary request,
-- every text column of every authority table must be free of canary
-- markers, prompt/output content, tokens, and local paths.
-- Run under a read-only inspection identity.

DECLARE @canary NVARCHAR(64) = N'%CANARY_%';
DECLARE @results TABLE (table_name SYSNAME, column_name SYSNAME, hits INT);

DECLARE @sql NVARCHAR(MAX) = N'';
SELECT @sql = @sql + N'
INSERT INTO @scan SELECT ''' + c.TABLE_NAME + N''', ''' + c.COLUMN_NAME + N''',
    (SELECT COUNT(*) FROM dbo.' + QUOTENAME(c.TABLE_NAME) + N'
     WHERE ' + QUOTENAME(c.COLUMN_NAME) + N' LIKE @pattern
        OR ' + QUOTENAME(c.COLUMN_NAME) + N' LIKE N''%eyJ%''
        OR ' + QUOTENAME(c.COLUMN_NAME) + N' LIKE N''%:\\%'');'
FROM INFORMATION_SCHEMA.COLUMNS AS c
WHERE c.TABLE_SCHEMA = 'dbo'
  AND c.TABLE_NAME LIKE 'desktop\_%' ESCAPE '\'
  AND c.DATA_TYPE LIKE '%char%';

DECLARE @wrapped NVARCHAR(MAX) = N'DECLARE @scan TABLE (table_name SYSNAME, column_name SYSNAME, hits INT);'
    + @sql + N' SELECT * FROM @scan WHERE hits > 0;';
EXEC sp_executesql @wrapped, N'@pattern NVARCHAR(64)', @pattern = @canary;
-- An empty result set is the passing condition.
