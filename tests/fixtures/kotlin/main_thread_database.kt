// Test fixture for MainThreadDatabaseDetector (AP027)
// Detects database operations on main thread

package com.example.antipattern

import android.os.Handler
import android.os.Looper
import androidx.room.*
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.withContext

// BAD: Database operations on main thread
class MainThreadDatabaseAccess {

    private val database: AppDatabase = TODO()
    private val userDao: UserDao = TODO()

    // BAD: Direct DAO call (blocks main thread)
    fun loadUsers(): List<User> {
        return userDao.getAllUsers()  // Blocks UI!
    }

    // BAD: Database query in onClick
    fun onButtonClick() {
        val users = database.userDao().getAllUsers()  // ANR risk!
        updateUI(users)
    }

    // BAD: Synchronous insert
    fun saveUser(user: User) {
        userDao.insert(user)  // Blocks main thread
    }

    // BAD: allowMainThreadQueries is a code smell
    fun createDatabase(context: Context): AppDatabase {
        return Room.databaseBuilder(context, AppDatabase::class.java, "app.db")
            .allowMainThreadQueries()  // BAD! Just hides the problem
            .build()
    }
}

// GOOD: Using coroutines with IO dispatcher
class CoroutinesDatabaseAccess {

    private val userDao: UserDao = TODO()

    // GOOD: Suspend function for database access
    suspend fun loadUsers(): List<User> {
        return withContext(Dispatchers.IO) {
            userDao.getAllUsers()
        }
    }

    // GOOD: Room suspend function
    suspend fun saveUser(user: User) {
        withContext(Dispatchers.IO) {
            userDao.insert(user)
        }
    }
}

// GOOD: Room DAO with suspend functions
@Dao
interface GoodUserDao {

    // GOOD: Suspend function - Room handles threading
    @Query("SELECT * FROM users")
    suspend fun getAllUsers(): List<User>

    // GOOD: Suspend insert
    @Insert
    suspend fun insert(user: User)

    // GOOD: Flow for reactive updates
    @Query("SELECT * FROM users")
    fun observeUsers(): Flow<List<User>>
}

// BAD: DAO without suspend (legacy pattern)
@Dao
interface BadUserDao {

    // BAD: Blocking call
    @Query("SELECT * FROM users")
    fun getAllUsers(): List<User>

    // BAD: Blocking insert
    @Insert
    fun insert(user: User)
}

// GOOD: Using LiveData (Room handles threading)
@Dao
interface LiveDataUserDao {

    // GOOD: LiveData is observed off main thread
    @Query("SELECT * FROM users")
    fun getAllUsers(): LiveData<List<User>>
}

// GOOD: Using background thread
class BackgroundThreadAccess {

    private val userDao: UserDao = TODO()
    private val handler = Handler(Looper.getMainLooper())

    fun loadUsersAsync(callback: (List<User>) -> Unit) {
        Thread {
            val users = userDao.getAllUsers()
            handler.post { callback(users) }
        }.start()
    }
}

// Supporting types
@Entity
data class User(
    @PrimaryKey val id: Int,
    val name: String
)

@Database(entities = [User::class], version = 1)
abstract class AppDatabase : RoomDatabase() {
    abstract fun userDao(): UserDao
}

interface UserDao {
    fun getAllUsers(): List<User>
    fun insert(user: User)
}

interface Context
class Flow<T>
class LiveData<T>
