package com.admin.config;

import lombok.extern.slf4j.Slf4j;
import org.springframework.boot.ApplicationArguments;
import org.springframework.boot.ApplicationRunner;
import org.springframework.stereotype.Component;

import javax.sql.DataSource;
import java.sql.Connection;
import java.sql.Statement;

/**
 * SQLite 数据库配置
 * 启用 WAL (Write-Ahead Logging) 模式以提高并发性能
 */
@Slf4j
@Component
public class SQLiteConfig implements ApplicationRunner {

    private final DataSource dataSource;

    public SQLiteConfig(DataSource dataSource) {
        this.dataSource = dataSource;
    }

    @Override
    public void run(ApplicationArguments args) throws Exception {
        try (Connection connection = dataSource.getConnection();
             Statement statement = connection.createStatement()) {
            
            statement.execute("PRAGMA journal_mode=WAL;");
            statement.execute("PRAGMA synchronous=NORMAL;");
            statement.execute("PRAGMA cache_size=-64000;"); // 64MB 缓存
            statement.execute("PRAGMA temp_store=MEMORY;");
            statement.execute("PRAGMA busy_timeout=5000;"); // 5秒超时
        } catch (Exception e) {
            log.error("Failed to configure SQLite database", e);
            throw e;
        }
    }
}

