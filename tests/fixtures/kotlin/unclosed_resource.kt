// Test fixture for UnclosedResourceDetector (AP026)
// Detects resources not properly closed

package com.example.antipattern

import android.database.Cursor
import java.io.*

// BAD: Resources not closed
class UnclosedResources {

    // BAD: Cursor not closed
    fun queryDatabase(db: SQLiteDatabase): List<User> {
        val cursor = db.rawQuery("SELECT * FROM users", null)
        val users = mutableListOf<User>()
        while (cursor.moveToNext()) {
            users.add(User(cursor.getString(0)))
        }
        // cursor.close() is missing!
        return users
    }

    // BAD: InputStream not closed
    fun readFile(file: File): String {
        val inputStream = FileInputStream(file)
        val content = inputStream.bufferedReader().readText()
        // inputStream.close() is missing!
        return content
    }

    // BAD: OutputStream not closed
    fun writeFile(file: File, content: String) {
        val outputStream = FileOutputStream(file)
        outputStream.write(content.toByteArray())
        // outputStream.close() is missing!
    }

    // BAD: BufferedReader not closed
    fun readLines(file: File): List<String> {
        val reader = BufferedReader(FileReader(file))
        val lines = reader.readLines()
        // reader.close() is missing!
        return lines
    }

    // BAD: Close in wrong place (not in finally)
    fun riskyRead(file: File): String {
        val stream = FileInputStream(file)
        val content = stream.bufferedReader().readText()  // Could throw!
        stream.close()  // Never reached if exception
        return content
    }
}

// GOOD: Using use {} block (Kotlin's try-with-resources)
class ProperResourceHandling {

    // GOOD: Cursor with use
    fun queryDatabase(db: SQLiteDatabase): List<User> {
        return db.rawQuery("SELECT * FROM users", null).use { cursor ->
            val users = mutableListOf<User>()
            while (cursor.moveToNext()) {
                users.add(User(cursor.getString(0)))
            }
            users
        }
    }

    // GOOD: InputStream with use
    fun readFile(file: File): String {
        return FileInputStream(file).use { stream ->
            stream.bufferedReader().readText()
        }
    }

    // GOOD: OutputStream with use
    fun writeFile(file: File, content: String) {
        FileOutputStream(file).use { stream ->
            stream.write(content.toByteArray())
        }
    }

    // GOOD: BufferedReader with use
    fun readLines(file: File): List<String> {
        return BufferedReader(FileReader(file)).use { reader ->
            reader.readLines()
        }
    }

    // GOOD: Multiple resources with nested use
    fun copyFile(source: File, dest: File) {
        FileInputStream(source).use { input ->
            FileOutputStream(dest).use { output ->
                input.copyTo(output)
            }
        }
    }
}

// GOOD: Using try-finally
class TryFinallyApproach {

    fun readWithFinally(file: File): String {
        val stream = FileInputStream(file)
        try {
            return stream.bufferedReader().readText()
        } finally {
            stream.close()
        }
    }
}

// OK: Resource returned to caller (caller's responsibility)
class ResourceFactory {

    fun createInputStream(file: File): InputStream {
        return FileInputStream(file)  // Caller must close
    }

    fun openCursor(db: SQLiteDatabase): Cursor {
        return db.rawQuery("SELECT * FROM users", null)  // Caller must close
    }
}

// Supporting types
data class User(val name: String)
interface SQLiteDatabase {
    fun rawQuery(sql: String, args: Array<String>?): Cursor
}
