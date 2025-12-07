// Test fixture: Room DAO write-only patterns
package com.example.fixtures.room

import androidx.room.*
import kotlinx.coroutines.flow.Flow

// Entity definitions
@Entity(tableName = "users")
data class User(
    @PrimaryKey val id: Long,
    val name: String,
    val email: String
)

@Entity(tableName = "read_history")
data class ReadHistory(
    @PrimaryKey val id: Long,
    val articleId: Long,
    val timestamp: Long
)

@Entity(tableName = "settings")
data class Settings(
    @PrimaryKey val key: String,
    val value: String
)

@Entity(tableName = "audit_log")
data class AuditLog(
    @PrimaryKey(autoGenerate = true) val id: Long = 0,
    val action: String,
    val timestamp: Long
)

// Case 1: Write-only DAO - @Insert but no corresponding @Query usage
@Dao
interface WriteOnlyDao {
    @Insert(onConflict = OnConflictStrategy.REPLACE)
    suspend fun saveReadHistory(history: ReadHistory)  // DEAD: Never queried

    @Insert
    suspend fun insertAuditLog(log: AuditLog)  // DEAD: Audit logs never read
}

// Case 2: NOT write-only - @Insert with corresponding @Query
@Dao
interface ReadWriteDao {
    @Insert(onConflict = OnConflictStrategy.REPLACE)
    suspend fun saveUser(user: User)

    @Query("SELECT * FROM users WHERE id = :userId")
    suspend fun getUserById(userId: Long): User?  // Query exists

    @Query("SELECT * FROM users")
    fun getAllUsers(): Flow<List<User>>  // Another query
}

// Case 3: Mixed DAO - some methods write-only, some used
@Dao
interface MixedDao {
    // DEAD: settings are saved but never read
    @Insert(onConflict = OnConflictStrategy.REPLACE)
    suspend fun saveSetting(setting: Settings)

    // NOT DEAD: user is saved and read
    @Insert(onConflict = OnConflictStrategy.REPLACE)
    suspend fun saveUser(user: User)

    @Query("SELECT * FROM users WHERE id = :userId")
    suspend fun getUserById(userId: Long): User?
}

// Case 4: DAO with @Delete and @Update but data never read
@Dao
interface DeleteOnlyDao {
    @Insert
    suspend fun insertItem(item: ReadHistory)

    @Delete
    suspend fun deleteItem(item: ReadHistory)

    @Update
    suspend fun updateItem(item: ReadHistory)

    // No @Query methods - all operations are write-only
}

// Case 5: DAO with complex queries - NOT write-only
@Dao
interface ComplexQueryDao {
    @Insert
    suspend fun insertHistory(history: ReadHistory)

    @Query("SELECT * FROM read_history ORDER BY timestamp DESC LIMIT :limit")
    fun getRecentHistory(limit: Int): Flow<List<ReadHistory>>

    @Query("SELECT COUNT(*) FROM read_history WHERE articleId = :articleId")
    suspend fun getReadCount(articleId: Long): Int

    @Query("SELECT * FROM read_history WHERE timestamp > :since")
    suspend fun getHistorySince(since: Long): List<ReadHistory>
}

// Case 6: Abstract DAO class pattern
@Dao
abstract class AbstractWriteOnlyDao {
    @Insert(onConflict = OnConflictStrategy.REPLACE)
    abstract suspend fun insertLog(log: AuditLog)  // DEAD: Never queried

    @Insert
    abstract suspend fun insertLogs(logs: List<AuditLog>)  // DEAD: Never queried
}

// Case 7: DAO with Transaction
@Dao
interface TransactionDao {
    @Insert
    suspend fun insertUsers(users: List<User>)

    @Query("DELETE FROM users")
    suspend fun deleteAllUsers()

    @Transaction
    suspend fun replaceAllUsers(users: List<User>) {
        deleteAllUsers()
        insertUsers(users)
    }

    // No read queries - this is write-only
}

// Case 8: DAO inherited from base interface
interface BaseDao<T> {
    @Insert(onConflict = OnConflictStrategy.REPLACE)
    suspend fun insert(item: T)

    @Delete
    suspend fun delete(item: T)
}

@Dao
interface UserDaoWithBase : BaseDao<User> {
    @Query("SELECT * FROM users")
    fun getAllUsers(): Flow<List<User>>  // Has query, so NOT write-only
}

@Dao
interface HistoryDaoWithBase : BaseDao<ReadHistory> {
    // No queries - write-only
}
