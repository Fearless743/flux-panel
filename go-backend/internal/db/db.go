package db

import (
	"embed"
	"fmt"
	"log/slog"
	"os"
	"path/filepath"
	"strings"
	"time"

	"github.com/jmoiron/sqlx"
	_ "modernc.org/sqlite"
)

//go:embed schema.sql data.sql
var sqlFS embed.FS

func Open(path string) (*sqlx.DB, error) {
	if err := os.MkdirAll(filepath.Dir(path), 0o755); err != nil {
		return nil, err
	}
	// modernc sqlite DSN
	dsn := fmt.Sprintf("file:%s?_pragma=busy_timeout(5000)&_pragma=journal_mode(WAL)&_pragma=synchronous(NORMAL)&_pragma=foreign_keys(ON)&_pragma=cache_size(-64000)", path)
	db, err := sqlx.Open("sqlite", dsn)
	if err != nil {
		return nil, err
	}
	db.SetMaxOpenConns(1) // SQLite 写串行更稳
	db.SetMaxIdleConns(1)
	db.SetConnMaxLifetime(time.Hour)

	if err := db.Ping(); err != nil {
		_ = db.Close()
		return nil, err
	}
	if err := applySQLFile(db, "schema.sql"); err != nil {
		_ = db.Close()
		return nil, fmt.Errorf("schema: %w", err)
	}
	if err := applySQLFile(db, "data.sql"); err != nil {
		_ = db.Close()
		return nil, fmt.Errorf("data: %w", err)
	}
	if err := migrate(db); err != nil {
		_ = db.Close()
		return nil, fmt.Errorf("migrate: %w", err)
	}
	slog.Info("database ready", "path", path)
	return db, nil
}

func applySQLFile(db *sqlx.DB, name string) error {
	b, err := sqlFS.ReadFile(name)
	if err != nil {
		return err
	}
	// 去掉行注释后按分号拆分（避免首条 CREATE 因前置 -- 注释被整段跳过）
	var cleaned strings.Builder
	for _, line := range strings.Split(string(b), "\n") {
		trim := strings.TrimSpace(line)
		if trim == "" || strings.HasPrefix(trim, "--") {
			continue
		}
		// 行内注释：仅当不在字符串中时粗暴截断，schema/data 无复杂字符串
		if i := strings.Index(line, "--"); i >= 0 {
			line = line[:i]
		}
		cleaned.WriteString(line)
		cleaned.WriteByte('\n')
	}
	parts := strings.Split(cleaned.String(), ";")
	for _, p := range parts {
		stmt := strings.TrimSpace(p)
		if stmt == "" {
			continue
		}
		if _, err := db.Exec(stmt); err != nil {
			return fmt.Errorf("%s exec %q: %w", name, truncate(stmt, 80), err)
		}
	}
	return nil
}

func truncate(s string, n int) string {
	if len(s) <= n {
		return s
	}
	return s[:n] + "..."
}

func Checkpoint(db *sqlx.DB) {
	if _, err := db.Exec(`PRAGMA wal_checkpoint(TRUNCATE)`); err != nil {
		slog.Warn("wal checkpoint failed", "err", err)
	}
}

func migrate(db *sqlx.DB) error {
	// 检查 chain_tunnel 表是否有 exit_node_ids 字段
	var count int
	err := db.Get(&count, `SELECT COUNT(*) FROM pragma_table_info('chain_tunnel') WHERE name='exit_node_ids'`)
	if err != nil {
		return fmt.Errorf("check exit_node_ids column: %w", err)
	}
	if count == 0 {
		// 添加 exit_node_ids 字段
		if _, err := db.Exec(`ALTER TABLE chain_tunnel ADD COLUMN exit_node_ids TEXT`); err != nil {
			return fmt.Errorf("add exit_node_ids column: %w", err)
		}
		slog.Info("migrated: added exit_node_ids column to chain_tunnel")
	}
	return nil
}
