package common

import (
	"database/sql"
	"fmt"
	"log"

	_ "github.com/mattn/go-sqlite3"
)

var DB *sql.DB

// InitDatabase 初始化sqlite3数据库
func InitDatabase() error {
	var err error
	DB, err = sql.Open("sqlite3", "data.db")
	if err != nil {
		return fmt.Errorf("连接数据库失败: %v", err)
	}

	if err = DB.Ping(); err != nil {
		return fmt.Errorf("数据库连接测试失败: %v", err)
	}

	// 创建表结构
	if err = createTables(); err != nil {
		return fmt.Errorf("创建表结构失败: %v", err)
	}

	return nil
}

// createTables 创建表结构
func createTables() error {
	tables := []string{
		`CREATE TABLE IF NOT EXISTS users (
			id INTEGER PRIMARY KEY AUTOINCREMENT,
			username VARCHAR(100) NOT NULL UNIQUE,
			password VARCHAR(100) NOT NULL,
			email VARCHAR(100),
			is_active BOOLEAN NOT NULL DEFAULT 1
		)`,
	}

	for _, table := range tables {
		if _, err := DB.Exec(table); err != nil {
			return err
		}
	}

	// 创建默认管理员用户
	//createDefaultUser()

	fmt.Println("✅ 数据库表创建完成")
	return nil
}

// createDefaultUser 创建默认管理员用户
func createDefaultUser() {
	var count int
	err := DB.QueryRow("SELECT COUNT(*) FROM users").Scan(&count)
	if err != nil || count > 0 {
		return
	}

	// 插入默认管理员用户
	_, err = DB.Exec(`INSERT INTO users (username, password, email, is_active) 
		VALUES (?, ?, ?, ?)`,
		"admin", "123456", "admin@example.com", true)

	if err != nil {
		log.Printf("创建默认用户失败: %v", err)
	} else {
		fmt.Println("✅ 创建默认管理员用户成功")
	}
}

// Insert 插入数据
func Insert(sql string, args ...interface{}) (sql.Result, error) {
	return DB.Exec(sql, args...)
}

// Update 更新数据
func Update(sql string, args ...interface{}) (sql.Result, error) {
	return DB.Exec(sql, args...)
}

// Delete 删除数据
func Delete(sql string, args ...interface{}) (sql.Result, error) {
	return DB.Exec(sql, args...)
}

// Select 查询数据
func Select(sql string, args ...interface{}) (*sql.Rows, error) {
	return DB.Query(sql, args...)
}
