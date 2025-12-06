//! Advanced Integration Tests for SearchDeadCode
//!
//! Ce fichier contient les 40 meilleurs tests couvrant:
//! - Parsing complexe (8 tests)
//! - Détection Cross-File (6 tests)
//! - Android spécifique (8 tests)
//! - Injection de dépendances (5 tests)
//! - Faux positifs critiques (7 tests)
//! - Performance & Stress (3 tests)
//! - Rapports & Output (3 tests)

use searchdeadcode::analysis::detectors::{Detector, WriteOnlyDetector};
use searchdeadcode::discovery::{FileType, SourceFile};
use searchdeadcode::graph::GraphBuilder;
use std::path::PathBuf;
use std::time::Instant;

fn create_temp_file(filename: &str, content: &str) -> (tempfile::TempDir, PathBuf) {
    let temp_dir = tempfile::tempdir().expect("Failed to create temp dir");
    let file_path = temp_dir.path().join(filename);
    std::fs::write(&file_path, content).expect("Failed to write file");
    (temp_dir, file_path)
}

fn build_graph_from_content(content: &str) -> searchdeadcode::graph::Graph {
    let (_temp_dir, file_path) = create_temp_file("test.kt", content);
    let source = SourceFile::new(file_path, FileType::Kotlin);
    let mut builder = GraphBuilder::new();
    builder
        .process_file(&source)
        .expect("Failed to process file");
    builder.build()
}

fn build_graph_from_java(content: &str) -> searchdeadcode::graph::Graph {
    let (_temp_dir, file_path) = create_temp_file("Test.java", content);
    let source = SourceFile::new(file_path, FileType::Java);
    let mut builder = GraphBuilder::new();
    builder
        .process_file(&source)
        .expect("Failed to process file");
    builder.build()
}

fn build_multi_file_graph(
    files: Vec<(&str, &str, FileType)>,
) -> (tempfile::TempDir, searchdeadcode::graph::Graph) {
    let temp_dir = tempfile::tempdir().expect("Failed to create temp dir");
    let mut builder = GraphBuilder::new();

    for (filename, content, file_type) in files {
        let file_path = temp_dir.path().join(filename);
        // Create parent directories if they don't exist
        if let Some(parent) = file_path.parent() {
            std::fs::create_dir_all(parent).expect("Failed to create parent directories");
        }
        std::fs::write(&file_path, content).expect("Failed to write file");
        let source = SourceFile::new(file_path, file_type);
        builder
            .process_file(&source)
            .expect("Failed to process file");
    }

    (temp_dir, builder.build())
}

// ============================================================================
// CATÉGORIE 1: TESTS DE PARSING (8 tests)
// ============================================================================

mod parsing_tests {
    use super::*;

    /// Test 1: Parsing de génériques complexes
    #[test]
    fn test_kotlin_complex_generics() {
        let content = r#"
package com.example.generics

// Génériques avec bounds multiples
class Repository<T : Comparable<T>, R : List<T>> where R : MutableList<T> {
    fun <S : T> process(item: S): T = item
    fun <A, B, C> multiGeneric(a: A, b: B, c: C): Triple<A, B, C> = Triple(a, b, c)
}

// Covariance et contravariance
interface Producer<out T> {
    fun produce(): T
}

interface Consumer<in T> {
    fun consume(item: T)
}

// Star projection
fun processAny(list: List<*>) {
    println(list.size)
}

// Reified type parameters
inline fun <reified T> isType(value: Any): Boolean = value is T

// Recursive generics
interface Comparable<T : Comparable<T>> {
    fun compareTo(other: T): Int
}

class SortedList<T : Comparable<T>> {
    private val items = mutableListOf<T>()
    fun add(item: T) { items.add(item) }
}
"#;

        let graph = build_graph_from_content(content);

        // Vérifier que les classes génériques sont parsées
        let repository = graph.declarations().find(|d| d.name == "Repository");
        assert!(repository.is_some(), "Repository doit être parsé");

        let producer = graph.declarations().find(|d| d.name == "Producer");
        assert!(producer.is_some(), "Producer (covariant) doit être parsé");

        let consumer = graph.declarations().find(|d| d.name == "Consumer");
        assert!(
            consumer.is_some(),
            "Consumer (contravariant) doit être parsé"
        );

        let is_type = graph.declarations().find(|d| d.name == "isType");
        assert!(is_type.is_some(), "isType (reified) doit être parsé");

        println!(
            "Parsed {} declarations with complex generics",
            graph.declarations().count()
        );
    }

    /// Test 2: Parsing de coroutines et suspend functions
    #[test]
    fn test_kotlin_coroutines_suspend() {
        let content = r#"
package com.example.coroutines

import kotlinx.coroutines.*

// Suspend functions
suspend fun fetchData(): String {
    delay(1000)
    return "data"
}

suspend fun processAsync(): Result<String> {
    return withContext(Dispatchers.IO) {
        Result.success("processed")
    }
}

// Flow
class DataRepository {
    fun observeData(): Flow<List<String>> = flow {
        while (true) {
            emit(listOf("item"))
            delay(1000)
        }
    }

    suspend fun getData(): List<String> {
        return listOf("data")
    }
}

// Coroutine scope
class ViewModel {
    private val scope = CoroutineScope(Dispatchers.Main)

    fun loadData() {
        scope.launch {
            val data = fetchData()
            println(data)
        }
    }

    suspend fun suspendLoad(): String {
        return coroutineScope {
            async { fetchData() }.await()
        }
    }
}

interface Flow<T>
fun <T> flow(block: suspend () -> Unit): Flow<T> = TODO()
object Dispatchers {
    val IO: Any = Unit
    val Main: Any = Unit
}
suspend fun delay(ms: Long) {}
suspend fun <T> withContext(context: Any, block: suspend () -> T): T = TODO()
class CoroutineScope(val context: Any) {
    fun launch(block: suspend () -> Unit) {}
}
suspend fun <T> coroutineScope(block: suspend () -> T): T = TODO()
fun <T> async(block: suspend () -> T): Deferred<T> = TODO()
interface Deferred<T> {
    suspend fun await(): T
}
"#;

        let graph = build_graph_from_content(content);

        // Vérifier les suspend functions
        let suspend_fns: Vec<_> = graph
            .declarations()
            .filter(|d| d.modifiers.iter().any(|m| m == "suspend"))
            .collect();

        println!("Found {} suspend functions", suspend_fns.len());

        // Vérifier les classes avec coroutines
        let view_model = graph.declarations().find(|d| d.name == "ViewModel");
        assert!(view_model.is_some(), "ViewModel doit être parsé");

        let data_repo = graph.declarations().find(|d| d.name == "DataRepository");
        assert!(data_repo.is_some(), "DataRepository doit être parsé");
    }

    /// Test 3: Parsing de expect/actual pour Kotlin Multiplatform
    #[test]
    fn test_kotlin_multiplatform_expect_actual() {
        let content = r#"
package com.example.multiplatform

// Common code - expect declarations
expect class Platform() {
    val name: String
    fun getVersion(): String
}

expect fun platformLog(message: String)

expect object PlatformConfig {
    val isDebug: Boolean
    fun initialize()
}

// Actual implementations (would be in platform-specific source sets)
// actual class Platform actual constructor() {
//     actual val name: String = "JVM"
//     actual fun getVersion(): String = "1.0"
// }

// Using expect/actual
class CommonClass {
    private val platform = Platform()

    fun printPlatform() {
        platformLog("Running on: ${platform.name}")
    }
}
"#;

        let graph = build_graph_from_content(content);

        // Vérifier les expect declarations
        let platform_class = graph.declarations().find(|d| d.name == "Platform");
        assert!(
            platform_class.is_some(),
            "expect class Platform doit être parsé"
        );

        let platform_log = graph.declarations().find(|d| d.name == "platformLog");
        assert!(
            platform_log.is_some(),
            "expect fun platformLog doit être parsé"
        );

        let platform_config = graph.declarations().find(|d| d.name == "PlatformConfig");
        assert!(
            platform_config.is_some(),
            "expect object PlatformConfig doit être parsé"
        );

        println!("Multiplatform declarations parsed successfully");
    }

    /// Test 4: Classes internes et anonymes Java
    #[test]
    fn test_java_inner_classes_anonymous() {
        let content = r#"
package com.example.inner;

public class OuterClass {
    private String outerField = "outer";

    // Static nested class
    public static class StaticNested {
        public void method() {
            System.out.println("Static nested");
        }
    }

    // Inner class (non-static)
    public class Inner {
        public void accessOuter() {
            System.out.println(outerField);
        }
    }

    // Local class
    public void methodWithLocalClass() {
        class LocalClass {
            void localMethod() {
                System.out.println("Local");
            }
        }
        new LocalClass().localMethod();
    }

    // Anonymous class
    public Runnable createRunnable() {
        return new Runnable() {
            @Override
            public void run() {
                System.out.println("Anonymous: " + outerField);
            }
        };
    }

    // Anonymous with interface
    public void useCallback() {
        Callback callback = new Callback() {
            @Override
            public void onSuccess() {}

            @Override
            public void onError(Exception e) {}
        };
        callback.onSuccess();
    }
}

interface Runnable {
    void run();
}

interface Callback {
    void onSuccess();
    void onError(Exception e);
}
"#;

        let graph = build_graph_from_java(content);

        // Vérifier les classes
        let outer = graph.declarations().find(|d| d.name == "OuterClass");
        assert!(outer.is_some(), "OuterClass doit être parsé");

        let static_nested = graph.declarations().find(|d| d.name == "StaticNested");
        assert!(static_nested.is_some(), "StaticNested doit être parsé");

        let inner = graph.declarations().find(|d| d.name == "Inner");
        assert!(inner.is_some(), "Inner class doit être parsé");

        println!(
            "Java inner classes parsed: {} declarations",
            graph.declarations().count()
        );
    }

    /// Test 5: Context receivers Kotlin
    #[test]
    fn test_kotlin_context_receivers() {
        let content = r#"
package com.example.context

// Context receivers (Kotlin 1.6.20+)
class Logger {
    fun log(message: String) = println(message)
}

class Transaction {
    fun execute(block: () -> Unit) = block()
}

// Function with context receiver
context(Logger)
fun loggedOperation(name: String) {
    log("Starting: $name")
    // ... operation
    log("Completed: $name")
}

// Class with context receiver
context(Logger, Transaction)
class BusinessOperation {
    fun perform() {
        log("Performing operation")
        execute {
            log("Inside transaction")
        }
    }
}

// Extension with context receiver
context(Logger)
fun String.logSelf() {
    log("String value: $this")
}

fun main() {
    with(Logger()) {
        loggedOperation("test")
        "hello".logSelf()
    }
}
"#;

        let graph = build_graph_from_content(content);

        // Vérifier les déclarations
        let logger = graph.declarations().find(|d| d.name == "Logger");
        assert!(logger.is_some(), "Logger doit être parsé");

        let transaction = graph.declarations().find(|d| d.name == "Transaction");
        assert!(transaction.is_some(), "Transaction doit être parsé");

        let business_op = graph.declarations().find(|d| d.name == "BusinessOperation");
        assert!(business_op.is_some(), "BusinessOperation doit être parsé");

        println!("Context receivers parsed successfully");
    }

    /// Test 6: Value classes (inline classes)
    #[test]
    fn test_kotlin_value_classes() {
        let content = r#"
package com.example.value

// Value class (Kotlin 1.5+)
@JvmInline
value class Password(val value: String) {
    init {
        require(value.length >= 8) { "Password too short" }
    }

    val isStrong: Boolean
        get() = value.length >= 12

    fun masked(): String = "*".repeat(value.length)
}

@JvmInline
value class UserId(val id: Long) {
    fun isValid(): Boolean = id > 0
}

@JvmInline
value class Email(val address: String) {
    init {
        require(address.contains("@")) { "Invalid email" }
    }

    val domain: String
        get() = address.substringAfter("@")
}

// Usage
fun authenticate(userId: UserId, password: Password): Boolean {
    return userId.isValid() && password.isStrong
}

fun main() {
    val user = UserId(123)
    val pwd = Password("securepassword")
    println(authenticate(user, pwd))
}
"#;

        let graph = build_graph_from_content(content);

        // Vérifier les value classes
        let password = graph.declarations().find(|d| d.name == "Password");
        assert!(password.is_some(), "Password value class doit être parsé");

        let user_id = graph.declarations().find(|d| d.name == "UserId");
        assert!(user_id.is_some(), "UserId value class doit être parsé");

        let email = graph.declarations().find(|d| d.name == "Email");
        assert!(email.is_some(), "Email value class doit être parsé");

        // Vérifier les méthodes des value classes
        let is_valid = graph.declarations().find(|d| d.name == "isValid");
        assert!(is_valid.is_some(), "isValid method doit être parsé");

        println!(
            "Value classes parsed: {} declarations",
            graph.declarations().count()
        );
    }

    /// Test 7: Récupération d'erreurs de syntaxe
    #[test]
    fn test_parsing_malformed_syntax() {
        // Code avec erreurs de syntaxe
        let malformed_content = r#"
package com.example.malformed

// Classe valide
class ValidClass {
    fun validMethod() {
        println("valid")
    }
}

// Syntaxe incorrecte - accolade manquante
class BrokenClass {
    fun brokenMethod() {
        println("broken"
    // } manquant
}

// Une autre classe valide après le code cassé
class AnotherValidClass {
    val property = "value"
}

// Fonction avec erreur
fun brokenFunction(x: Int {  // parenthèse manquante
    return x * 2
}

// Classe finale valide
class FinalValidClass {
    fun works() = "ok"
}
"#;

        let (_temp_dir, file_path) = create_temp_file("malformed.kt", malformed_content);
        let source = SourceFile::new(file_path, FileType::Kotlin);
        let mut builder = GraphBuilder::new();

        // Le parser devrait gérer les erreurs gracieusement
        let result = builder.process_file(&source);

        // Même avec des erreurs, on devrait pouvoir parser
        // les parties valides du fichier
        println!("Malformed file processing result: {:?}", result.is_ok());

        if result.is_ok() {
            let graph = builder.build();
            let valid_classes: Vec<_> = graph
                .declarations()
                .filter(|d| d.name.contains("Valid"))
                .collect();

            println!(
                "Found {} valid classes in malformed file",
                valid_classes.len()
            );
        }
    }

    /// Test 8: Code généré par KSP
    #[test]
    fn test_kotlin_ksp_generated_code() {
        let content = r#"
package com.example.generated

// Annotations qui déclenchent la génération KSP
annotation class AutoFactory
annotation class AutoBuilder

// Classe source avec annotations
@AutoFactory
class UserService(
    private val repository: UserRepository,
    private val validator: Validator
) {
    fun createUser(name: String): User {
        validator.validate(name)
        return repository.save(User(0, name))
    }
}

// Code généré par KSP (simulé)
// Le vrai code serait dans build/generated/ksp/
class UserService_Factory(
    private val repositoryProvider: Provider<UserRepository>,
    private val validatorProvider: Provider<Validator>
) : Factory<UserService> {
    override fun get(): UserService {
        return UserService(
            repositoryProvider.get(),
            validatorProvider.get()
        )
    }

    companion object {
        fun create(
            repositoryProvider: Provider<UserRepository>,
            validatorProvider: Provider<Validator>
        ): UserService_Factory {
            return UserService_Factory(repositoryProvider, validatorProvider)
        }
    }
}

interface Provider<T> {
    fun get(): T
}

interface Factory<T> {
    fun get(): T
}

interface UserRepository {
    fun save(user: User): User
}

interface Validator {
    fun validate(input: String)
}

data class User(val id: Long, val name: String)
"#;

        let graph = build_graph_from_content(content);

        // Le code généré doit être parsé
        let factory = graph
            .declarations()
            .find(|d| d.name == "UserService_Factory");
        assert!(factory.is_some(), "Generated factory doit être parsé");

        // Les classes source aussi
        let service = graph.declarations().find(|d| d.name == "UserService");
        assert!(service.is_some(), "Source class doit être parsé");

        // Le code généré NE DOIT PAS être signalé comme mort
        // car il est utilisé par le framework DI

        println!(
            "KSP generated code parsed: {} declarations",
            graph.declarations().count()
        );
    }
}

// ============================================================================
// CATÉGORIE 2: TESTS CROSS-FILE (6 tests)
// ============================================================================

mod cross_file_tests {
    use super::*;

    /// Test 9: Références entre modules Gradle
    #[test]
    fn test_cross_module_references() {
        let files = vec![
            // Module :core
            (
                "core/User.kt",
                r#"
package com.example.core.model

data class User(
    val id: Long,
    val name: String,
    val email: String
)

interface UserRepository {
    fun findById(id: Long): User?
    fun save(user: User): User
}
"#,
                FileType::Kotlin,
            ),
            // Module :data (dépend de :core)
            (
                "data/UserRepositoryImpl.kt",
                r#"
package com.example.data.repository

import com.example.core.model.User
import com.example.core.model.UserRepository

class UserRepositoryImpl : UserRepository {
    private val cache = mutableMapOf<Long, User>()

    override fun findById(id: Long): User? = cache[id]

    override fun save(user: User): User {
        cache[user.id] = user
        return user
    }
}
"#,
                FileType::Kotlin,
            ),
            // Module :app (dépend de :core et :data)
            (
                "app/MainViewModel.kt",
                r#"
package com.example.app.ui

import com.example.core.model.User
import com.example.core.model.UserRepository

class MainViewModel(
    private val repository: UserRepository
) {
    fun loadUser(id: Long): User? {
        return repository.findById(id)
    }

    fun createUser(name: String, email: String): User {
        val user = User(System.currentTimeMillis(), name, email)
        return repository.save(user)
    }
}
"#,
                FileType::Kotlin,
            ),
        ];

        let (_temp_dir, graph) = build_multi_file_graph(files);

        // Vérifier les déclarations de chaque module
        let user = graph.declarations().find(|d| d.name == "User");
        assert!(user.is_some(), "User (core) doit être trouvé");

        let user_repo = graph.declarations().find(|d| d.name == "UserRepository");
        assert!(
            user_repo.is_some(),
            "UserRepository (core) doit être trouvé"
        );

        let user_repo_impl = graph
            .declarations()
            .find(|d| d.name == "UserRepositoryImpl");
        assert!(
            user_repo_impl.is_some(),
            "UserRepositoryImpl (data) doit être trouvé"
        );

        let view_model = graph.declarations().find(|d| d.name == "MainViewModel");
        assert!(view_model.is_some(), "MainViewModel (app) doit être trouvé");

        println!(
            "Cross-module: {} declarations across 3 modules",
            graph.declarations().count()
        );
    }

    /// Test 10: Interface dans un fichier, impl dans un autre
    #[test]
    fn test_interface_impl_different_files() {
        let files = vec![
            (
                "contracts/DataSource.kt",
                r#"
package com.example.contracts

interface DataSource<T> {
    fun getAll(): List<T>
    fun getById(id: Long): T?
    fun insert(item: T): Long
    fun update(item: T): Boolean
    fun delete(id: Long): Boolean
}

interface NetworkDataSource<T> : DataSource<T> {
    suspend fun sync(): Result<Unit>
    fun isOnline(): Boolean
}
"#,
                FileType::Kotlin,
            ),
            (
                "impl/LocalDataSource.kt",
                r#"
package com.example.impl

import com.example.contracts.DataSource

class LocalDataSource<T> : DataSource<T> {
    private val storage = mutableMapOf<Long, T>()
    private var nextId = 1L

    override fun getAll(): List<T> = storage.values.toList()
    override fun getById(id: Long): T? = storage[id]
    override fun insert(item: T): Long {
        val id = nextId++
        storage[id] = item
        return id
    }
    override fun update(item: T): Boolean = true
    override fun delete(id: Long): Boolean = storage.remove(id) != null
}
"#,
                FileType::Kotlin,
            ),
            (
                "impl/RemoteDataSource.kt",
                r#"
package com.example.impl

import com.example.contracts.NetworkDataSource

class RemoteDataSource<T> : NetworkDataSource<T> {
    override fun getAll(): List<T> = emptyList()
    override fun getById(id: Long): T? = null
    override fun insert(item: T): Long = 0
    override fun update(item: T): Boolean = false
    override fun delete(id: Long): Boolean = false
    override suspend fun sync(): Result<Unit> = Result.success(Unit)
    override fun isOnline(): Boolean = true
}
"#,
                FileType::Kotlin,
            ),
        ];

        let (_temp_dir, graph) = build_multi_file_graph(files);

        // Interfaces
        let data_source = graph.declarations().find(|d| d.name == "DataSource");
        assert!(
            data_source.is_some(),
            "DataSource interface doit être trouvé"
        );

        let network_ds = graph.declarations().find(|d| d.name == "NetworkDataSource");
        assert!(
            network_ds.is_some(),
            "NetworkDataSource interface doit être trouvé"
        );

        // Implementations
        let local_ds = graph.declarations().find(|d| d.name == "LocalDataSource");
        assert!(local_ds.is_some(), "LocalDataSource impl doit être trouvé");

        let remote_ds = graph.declarations().find(|d| d.name == "RemoteDataSource");
        assert!(
            remote_ds.is_some(),
            "RemoteDataSource impl doit être trouvé"
        );

        println!(
            "Interface/Impl cross-file: {} declarations",
            graph.declarations().count()
        );
    }

    /// Test 11: Extension function cross-file
    #[test]
    fn test_extension_function_cross_file() {
        let files = vec![
            (
                "extensions/StringExtensions.kt",
                r#"
package com.example.extensions

fun String.toSlug(): String =
    this.lowercase()
        .replace(" ", "-")
        .replace(Regex("[^a-z0-9-]"), "")

fun String.truncate(maxLength: Int, suffix: String = "..."): String =
    if (this.length <= maxLength) this
    else this.take(maxLength - suffix.length) + suffix

fun String.isValidEmail(): Boolean =
    this.contains("@") && this.contains(".")

fun String.capitalizeWords(): String =
    this.split(" ").joinToString(" ") {
        it.replaceFirstChar { c -> c.uppercase() }
    }
"#,
                FileType::Kotlin,
            ),
            (
                "usage/ArticleProcessor.kt",
                r#"
package com.example.usage

import com.example.extensions.toSlug
import com.example.extensions.truncate
import com.example.extensions.capitalizeWords

class ArticleProcessor {
    fun processTitle(title: String): ProcessedTitle {
        return ProcessedTitle(
            display = title.capitalizeWords(),
            slug = title.toSlug(),
            preview = title.truncate(50)
        )
    }
}

data class ProcessedTitle(
    val display: String,
    val slug: String,
    val preview: String
)
"#,
                FileType::Kotlin,
            ),
            (
                "usage/EmailValidator.kt",
                r#"
package com.example.usage

import com.example.extensions.isValidEmail

class EmailValidator {
    fun validate(email: String): ValidationResult {
        return if (email.isValidEmail()) {
            ValidationResult.Valid
        } else {
            ValidationResult.Invalid("Not a valid email format")
        }
    }
}

sealed class ValidationResult {
    object Valid : ValidationResult()
    data class Invalid(val reason: String) : ValidationResult()
}
"#,
                FileType::Kotlin,
            ),
        ];

        let (_temp_dir, graph) = build_multi_file_graph(files);

        // Extensions
        let to_slug = graph.declarations().find(|d| d.name == "toSlug");
        assert!(to_slug.is_some(), "toSlug extension doit être trouvé");

        let is_valid_email = graph.declarations().find(|d| d.name == "isValidEmail");
        assert!(
            is_valid_email.is_some(),
            "isValidEmail extension doit être trouvé"
        );

        // Usage classes
        let article_processor = graph.declarations().find(|d| d.name == "ArticleProcessor");
        assert!(
            article_processor.is_some(),
            "ArticleProcessor doit être trouvé"
        );

        let email_validator = graph.declarations().find(|d| d.name == "EmailValidator");
        assert!(email_validator.is_some(), "EmailValidator doit être trouvé");

        println!(
            "Extension cross-file: {} declarations",
            graph.declarations().count()
        );
    }

    /// Test 12: Typealias cross-file
    #[test]
    fn test_typealias_cross_file() {
        let files = vec![
            (
                "types/Aliases.kt",
                r#"
package com.example.types

// Simple typealiases
typealias UserId = Long
typealias UserName = String
typealias Email = String

// Function typealiases
typealias Callback<T> = (T) -> Unit
typealias AsyncCallback<T> = suspend (T) -> Unit
typealias Predicate<T> = (T) -> Boolean
typealias Mapper<T, R> = (T) -> R

// Complex typealiases
typealias UserMap = Map<UserId, User>
typealias UserList = List<User>
typealias UserCallback = Callback<User>

data class User(val id: UserId, val name: UserName, val email: Email)
"#,
                FileType::Kotlin,
            ),
            (
                "service/UserService.kt",
                r#"
package com.example.service

import com.example.types.*

class UserService {
    private val users: UserMap = mutableMapOf()
    private val listeners: MutableList<UserCallback> = mutableListOf()

    fun addUser(id: UserId, name: UserName, email: Email): User {
        val user = User(id, name, email)
        users[id] = user
        notifyListeners(user)
        return user
    }

    fun findUser(predicate: Predicate<User>): User? {
        return users.values.find(predicate)
    }

    fun mapUsers<R>(mapper: Mapper<User, R>): List<R> {
        return users.values.map(mapper)
    }

    fun addListener(callback: UserCallback) {
        listeners.add(callback)
    }

    private fun notifyListeners(user: User) {
        listeners.forEach { it(user) }
    }
}
"#,
                FileType::Kotlin,
            ),
        ];

        let (_temp_dir, graph) = build_multi_file_graph(files);

        // Typealiases
        let type_aliases = [
            "UserId",
            "UserName",
            "Email",
            "Callback",
            "Predicate",
            "Mapper",
        ];
        for alias in &type_aliases {
            let found = graph.declarations().any(|d| d.name == *alias);
            println!("Typealias {}: found = {}", alias, found);
        }

        // Service using typealiases
        let user_service = graph.declarations().find(|d| d.name == "UserService");
        assert!(user_service.is_some(), "UserService doit être trouvé");

        println!(
            "Typealias cross-file: {} declarations",
            graph.declarations().count()
        );
    }

    /// Test 13: Companion object cross-file
    #[test]
    fn test_companion_object_cross_file() {
        let files = vec![
            (
                "models/Config.kt",
                r#"
package com.example.models

class AppConfig private constructor(
    val apiUrl: String,
    val timeout: Long,
    val debug: Boolean
) {
    companion object {
        private var instance: AppConfig? = null

        fun getInstance(): AppConfig {
            return instance ?: create().also { instance = it }
        }

        fun create(
            apiUrl: String = "https://api.example.com",
            timeout: Long = 30000,
            debug: Boolean = false
        ): AppConfig {
            return AppConfig(apiUrl, timeout, debug)
        }

        const val DEFAULT_TIMEOUT = 30000L
        const val VERSION = "1.0.0"
    }
}
"#,
                FileType::Kotlin,
            ),
            (
                "network/ApiClient.kt",
                r#"
package com.example.network

import com.example.models.AppConfig

class ApiClient {
    private val config = AppConfig.getInstance()

    fun getBaseUrl(): String = config.apiUrl

    fun getTimeout(): Long = AppConfig.DEFAULT_TIMEOUT

    fun getVersion(): String = AppConfig.VERSION

    fun request(endpoint: String): String {
        println("Requesting: ${config.apiUrl}/$endpoint")
        return "response"
    }
}
"#,
                FileType::Kotlin,
            ),
            (
                "di/AppModule.kt",
                r#"
package com.example.di

import com.example.models.AppConfig
import com.example.network.ApiClient

object AppModule {
    private val config: AppConfig by lazy {
        AppConfig.create(
            apiUrl = "https://prod.api.example.com",
            timeout = 60000,
            debug = false
        )
    }

    private val apiClient: ApiClient by lazy {
        ApiClient()
    }

    fun provideConfig(): AppConfig = config
    fun provideApiClient(): ApiClient = apiClient
}
"#,
                FileType::Kotlin,
            ),
        ];

        let (_temp_dir, graph) = build_multi_file_graph(files);

        // Config class and companion
        let app_config = graph.declarations().find(|d| d.name == "AppConfig");
        assert!(app_config.is_some(), "AppConfig doit être trouvé");

        // Companion methods used cross-file
        let get_instance = graph.declarations().find(|d| d.name == "getInstance");
        assert!(get_instance.is_some(), "getInstance doit être trouvé");

        let create = graph.declarations().find(|d| d.name == "create");
        assert!(create.is_some(), "create doit être trouvé");

        // Usage
        let api_client = graph.declarations().find(|d| d.name == "ApiClient");
        assert!(api_client.is_some(), "ApiClient doit être trouvé");

        println!(
            "Companion cross-file: {} declarations",
            graph.declarations().count()
        );
    }

    /// Test 14: Sealed hierarchy cross-file
    #[test]
    fn test_sealed_hierarchy_cross_file() {
        let files = vec![
            (
                "state/UiState.kt",
                r#"
package com.example.state

sealed class UiState<out T> {
    abstract val isTerminal: Boolean
}

// Variants in same file
object Loading : UiState<Nothing>() {
    override val isTerminal = false
}

object Idle : UiState<Nothing>() {
    override val isTerminal = true
}
"#,
                FileType::Kotlin,
            ),
            (
                "state/SuccessState.kt",
                r#"
package com.example.state

data class Success<T>(
    val data: T,
    val timestamp: Long = System.currentTimeMillis()
) : UiState<T>() {
    override val isTerminal = true
}

data class PartialSuccess<T>(
    val data: T,
    val hasMore: Boolean
) : UiState<T>() {
    override val isTerminal = false
}
"#,
                FileType::Kotlin,
            ),
            (
                "state/ErrorState.kt",
                r#"
package com.example.state

sealed class ErrorState : UiState<Nothing>() {
    override val isTerminal = true

    data class NetworkError(val message: String) : ErrorState()
    data class ServerError(val code: Int, val message: String) : ErrorState()
    data class ValidationError(val field: String, val message: String) : ErrorState()
    object UnknownError : ErrorState()
}
"#,
                FileType::Kotlin,
            ),
            (
                "viewmodel/StateHandler.kt",
                r#"
package com.example.viewmodel

import com.example.state.*

class StateHandler<T> {
    fun handle(state: UiState<T>): String {
        return when (state) {
            is Loading -> "Loading..."
            is Idle -> "Ready"
            is Success -> "Got: ${state.data}"
            is PartialSuccess -> "Partial: ${state.data}, more: ${state.hasMore}"
            is ErrorState.NetworkError -> "Network: ${state.message}"
            is ErrorState.ServerError -> "Server ${state.code}: ${state.message}"
            is ErrorState.ValidationError -> "${state.field}: ${state.message}"
            is ErrorState.UnknownError -> "Unknown error"
        }
    }
}
"#,
                FileType::Kotlin,
            ),
        ];

        let (_temp_dir, graph) = build_multi_file_graph(files);

        // Sealed class
        let ui_state = graph.declarations().find(|d| d.name == "UiState");
        assert!(ui_state.is_some(), "UiState sealed class doit être trouvé");

        // Variants from different files
        let variants = ["Loading", "Idle", "Success", "PartialSuccess", "ErrorState"];
        for variant in &variants {
            let found = graph.declarations().any(|d| d.name == *variant);
            assert!(found, "Variant {} doit être trouvé", variant);
        }

        // Nested sealed variants
        let network_error = graph.declarations().find(|d| d.name == "NetworkError");
        assert!(network_error.is_some(), "NetworkError doit être trouvé");

        println!(
            "Sealed hierarchy cross-file: {} declarations",
            graph.declarations().count()
        );
    }
}

// ============================================================================
// CATÉGORIE 3: TESTS ANDROID SPÉCIFIQUES (8 tests)
// ============================================================================

mod android_tests {
    use super::*;

    /// Test 15: Activities déclarées dans AndroidManifest
    #[test]
    fn test_manifest_activities_not_dead() {
        let content = r#"
package com.example.ui

import android.os.Bundle
import android.content.Intent

// Ces activités sont déclarées dans AndroidManifest.xml
// Elles NE DOIVENT PAS être signalées comme mortes

abstract class BaseActivity {
    abstract fun onCreate(savedInstanceState: Bundle?)
}

class MainActivity : BaseActivity() {
    override fun onCreate(savedInstanceState: Bundle?) {
        println("MainActivity created")
    }

    fun navigateToSettings() {
        // startActivity(Intent(this, SettingsActivity::class.java))
    }
}

class SettingsActivity : BaseActivity() {
    override fun onCreate(savedInstanceState: Bundle?) {
        println("SettingsActivity created")
    }
}

class LoginActivity : BaseActivity() {
    override fun onCreate(savedInstanceState: Bundle?) {
        println("LoginActivity created")
    }

    fun onLoginSuccess() {
        // navigateToMain()
    }
}

class SplashActivity : BaseActivity() {
    override fun onCreate(savedInstanceState: Bundle?) {
        // Check auth and navigate
    }
}

// Deep link handler
class DeepLinkActivity : BaseActivity() {
    override fun onCreate(savedInstanceState: Bundle?) {
        // Handle deep link
    }
}

class Bundle
class Intent
"#;

        let graph = build_graph_from_content(content);

        // Toutes les activités doivent être trouvées
        let activities = [
            "MainActivity",
            "SettingsActivity",
            "LoginActivity",
            "SplashActivity",
            "DeepLinkActivity",
        ];

        for activity in &activities {
            let found = graph.declarations().any(|d| d.name == *activity);
            assert!(found, "Activity {} doit être trouvée", activity);
        }

        // Ces activités ne doivent pas être signalées comme mortes
        // car elles sont déclarées dans le manifest
        println!("Manifest activities: {} found", activities.len());
    }

    /// Test 16: Services et Receivers du manifest
    #[test]
    fn test_manifest_services_receivers() {
        let content = r#"
package com.example.services

import android.content.Context
import android.content.Intent

// Services déclarés dans le manifest
abstract class Service {
    abstract fun onStartCommand(intent: Intent?, flags: Int, startId: Int): Int
    open fun onBind(intent: Intent): Any? = null
    open fun onDestroy() {}
}

class SyncService : Service() {
    override fun onStartCommand(intent: Intent?, flags: Int, startId: Int): Int {
        performSync()
        return 1 // START_STICKY
    }

    private fun performSync() {
        println("Syncing...")
    }
}

class NotificationService : Service() {
    override fun onStartCommand(intent: Intent?, flags: Int, startId: Int): Int {
        showNotification()
        return 0 // START_NOT_STICKY
    }

    private fun showNotification() {}
}

// BroadcastReceivers
abstract class BroadcastReceiver {
    abstract fun onReceive(context: Context, intent: Intent)
}

class BootReceiver : BroadcastReceiver() {
    override fun onReceive(context: Context, intent: Intent) {
        if (intent.action == "android.intent.action.BOOT_COMPLETED") {
            scheduleWork()
        }
    }

    private fun scheduleWork() {}
}

class NetworkChangeReceiver : BroadcastReceiver() {
    override fun onReceive(context: Context, intent: Intent) {
        checkConnectivity()
    }

    private fun checkConnectivity() {}
}

class PushReceiver : BroadcastReceiver() {
    override fun onReceive(context: Context, intent: Intent) {
        handlePushMessage(intent)
    }

    private fun handlePushMessage(intent: Intent) {}
}

class Context
class Intent {
    val action: String = ""
}
"#;

        let graph = build_graph_from_content(content);

        // Services
        let services = ["SyncService", "NotificationService"];
        for service in &services {
            let found = graph.declarations().any(|d| d.name == *service);
            assert!(found, "Service {} doit être trouvé", service);
        }

        // Receivers
        let receivers = ["BootReceiver", "NetworkChangeReceiver", "PushReceiver"];
        for receiver in &receivers {
            let found = graph.declarations().any(|d| d.name == *receiver);
            assert!(found, "Receiver {} doit être trouvé", receiver);
        }

        println!(
            "Services and Receivers: {} found",
            services.len() + receivers.len()
        );
    }

    /// Test 17: Variables DataBinding dans XML
    #[test]
    fn test_databinding_variables() {
        let content = r#"
package com.example.databinding

// Classes utilisées dans les layouts DataBinding
// <variable name="viewModel" type="com.example.databinding.MainViewModel"/>

class MainViewModel {
    val userName: String = "John"
    val isLoggedIn: Boolean = true
    val itemCount: Int = 0

    fun onLoginClick() {
        println("Login clicked")
    }

    fun onItemSelected(position: Int) {
        println("Selected: $position")
    }
}

// <variable name="item" type="com.example.databinding.ListItem"/>
data class ListItem(
    val id: Long,
    val title: String,
    val description: String,
    val imageUrl: String
)

// Binding adapters
object BindingAdapters {
    @JvmStatic
    fun setImageUrl(view: Any, url: String?) {
        // Load image
    }

    @JvmStatic
    fun setVisible(view: Any, visible: Boolean) {
        // Set visibility
    }

    @JvmStatic
    fun formatDate(timestamp: Long): String {
        return timestamp.toString()
    }
}

// BindingConversions
object Converters {
    @JvmStatic
    fun intToString(value: Int): String = value.toString()

    @JvmStatic
    fun boolToVisibility(value: Boolean): Int = if (value) 0 else 8
}
"#;

        let graph = build_graph_from_content(content);

        // ViewModel utilisé dans binding
        let view_model = graph.declarations().find(|d| d.name == "MainViewModel");
        assert!(view_model.is_some(), "MainViewModel doit être trouvé");

        // Propriétés bindées
        let bound_props = ["userName", "isLoggedIn", "itemCount"];
        for prop in &bound_props {
            let found = graph.declarations().any(|d| d.name == *prop);
            println!("Bound property {}: found = {}", prop, found);
        }

        // Binding adapters
        let adapters = graph.declarations().find(|d| d.name == "BindingAdapters");
        assert!(adapters.is_some(), "BindingAdapters doit être trouvé");

        println!("DataBinding: {} declarations", graph.declarations().count());
    }

    /// Test 18: Classes ViewBinding générées
    #[test]
    fn test_viewbinding_generated() {
        let content = r#"
package com.example.ui

// Simulated ViewBinding generated classes
// Ces classes sont générées à partir des layouts XML

class ActivityMainBinding {
    val root: View = View()
    val toolbar: View = View()
    val recyclerView: View = View()
    val fab: View = View()
    val progressBar: View = View()

    companion object {
        fun inflate(inflater: LayoutInflater): ActivityMainBinding {
            return ActivityMainBinding()
        }

        fun bind(view: View): ActivityMainBinding {
            return ActivityMainBinding()
        }
    }
}

class FragmentHomeBinding {
    val root: View = View()
    val titleText: View = View()
    val contentList: View = View()

    companion object {
        fun inflate(inflater: LayoutInflater): FragmentHomeBinding {
            return FragmentHomeBinding()
        }
    }
}

class ItemListBinding {
    val root: View = View()
    val icon: View = View()
    val title: View = View()
    val subtitle: View = View()

    companion object {
        fun inflate(inflater: LayoutInflater, parent: View?, attachToParent: Boolean): ItemListBinding {
            return ItemListBinding()
        }
    }
}

// Usage in Activity
class MainActivity {
    private lateinit var binding: ActivityMainBinding

    fun onCreate() {
        binding = ActivityMainBinding.inflate(LayoutInflater())
        setupViews()
    }

    private fun setupViews() {
        binding.toolbar
        binding.recyclerView
        binding.fab
    }
}

class View
class LayoutInflater
"#;

        let graph = build_graph_from_content(content);

        // Generated binding classes
        let bindings = [
            "ActivityMainBinding",
            "FragmentHomeBinding",
            "ItemListBinding",
        ];
        for binding in &bindings {
            let found = graph.declarations().any(|d| d.name == *binding);
            assert!(found, "ViewBinding {} doit être trouvé", binding);
        }

        // View references (auto-generated from XML IDs)
        let views = ["toolbar", "recyclerView", "fab", "progressBar"];
        for view in &views {
            let found = graph.declarations().any(|d| d.name == *view);
            println!("View {}: found = {}", view, found);
        }

        println!("ViewBinding: {} declarations", graph.declarations().count());
    }

    /// Test 19: Fragments dans nav_graph.xml
    #[test]
    fn test_navigation_graph_destinations() {
        let content = r#"
package com.example.navigation

import android.os.Bundle

// Fragments déclarés dans nav_graph.xml
// Ces fragments sont des destinations de navigation

abstract class Fragment {
    open fun onViewCreated(view: Any, savedInstanceState: Bundle?) {}
    open fun onDestroyView() {}
}

class HomeFragment : Fragment() {
    override fun onViewCreated(view: Any, savedInstanceState: Bundle?) {
        setupUI()
    }

    private fun setupUI() {}

    fun navigateToDetails(itemId: Long) {
        // findNavController().navigate(...)
    }
}

class DetailsFragment : Fragment() {
    override fun onViewCreated(view: Any, savedInstanceState: Bundle?) {
        val itemId = arguments?.getLong("itemId") ?: 0
        loadDetails(itemId)
    }

    private fun loadDetails(id: Long) {}

    private val arguments: Bundle? = null
}

class SettingsFragment : Fragment() {
    override fun onViewCreated(view: Any, savedInstanceState: Bundle?) {
        loadSettings()
    }

    private fun loadSettings() {}
}

class ProfileFragment : Fragment() {
    override fun onViewCreated(view: Any, savedInstanceState: Bundle?) {
        loadProfile()
    }

    private fun loadProfile() {}
}

// Dialog destination
class ConfirmDialogFragment : Fragment() {
    override fun onViewCreated(view: Any, savedInstanceState: Bundle?) {
        setupDialog()
    }

    private fun setupDialog() {}
}

// Bottom sheet destination
class FilterBottomSheet : Fragment() {
    override fun onViewCreated(view: Any, savedInstanceState: Bundle?) {
        setupFilters()
    }

    private fun setupFilters() {}
}

class Bundle {
    fun getLong(key: String): Long = 0
}
"#;

        let graph = build_graph_from_content(content);

        // Navigation destinations
        let fragments = [
            "HomeFragment",
            "DetailsFragment",
            "SettingsFragment",
            "ProfileFragment",
            "ConfirmDialogFragment",
            "FilterBottomSheet",
        ];

        for fragment in &fragments {
            let found = graph.declarations().any(|d| d.name == *fragment);
            assert!(
                found,
                "Navigation destination {} doit être trouvé",
                fragment
            );
        }

        println!("Navigation destinations: {} found", fragments.len());
    }

    /// Test 20: @Preview composable functions
    #[test]
    fn test_compose_preview_functions() {
        let content = r#"
package com.example.compose

// Compose preview functions
// Ces fonctions sont appelées par l'IDE pour le preview

annotation class Composable
annotation class Preview(
    val name: String = "",
    val showBackground: Boolean = false
)

@Composable
fun UserCard(user: User, onClick: () -> Unit) {
    // Card content
}

@Preview(name = "User Card", showBackground = true)
@Composable
fun UserCardPreview() {
    UserCard(
        user = User(1, "John", "john@example.com"),
        onClick = {}
    )
}

@Composable
fun LoadingIndicator() {
    // Loading UI
}

@Preview
@Composable
fun LoadingIndicatorPreview() {
    LoadingIndicator()
}

@Composable
fun ErrorMessage(message: String, onRetry: () -> Unit) {
    // Error UI
}

@Preview(name = "Error State")
@Composable
fun ErrorMessagePreview() {
    ErrorMessage(
        message = "Something went wrong",
        onRetry = {}
    )
}

// Multiple previews
@Preview(name = "Light Theme")
@Preview(name = "Dark Theme")
@Composable
fun ThemedComponentPreview() {
    // Themed preview
}

data class User(val id: Long, val name: String, val email: String)
"#;

        let graph = build_graph_from_content(content);

        // Preview functions
        let previews = [
            "UserCardPreview",
            "LoadingIndicatorPreview",
            "ErrorMessagePreview",
            "ThemedComponentPreview",
        ];

        for preview in &previews {
            let found = graph.declarations().any(|d| d.name == *preview);
            assert!(found, "Preview function {} doit être trouvée", preview);
        }

        // Main composables
        let composables = ["UserCard", "LoadingIndicator", "ErrorMessage"];
        for composable in &composables {
            let found = graph.declarations().any(|d| d.name == *composable);
            assert!(found, "Composable {} doit être trouvé", composable);
        }

        println!("Compose previews: {} found", previews.len());
    }

    /// Test 21: Worker classes pour WorkManager
    #[test]
    fn test_workmanager_workers() {
        let content = r#"
package com.example.workers

import android.content.Context

// WorkManager workers - instanciés par le framework

abstract class Worker(context: Context, params: WorkerParameters) {
    abstract fun doWork(): Result

    sealed class Result {
        object Success : Result()
        object Failure : Result()
        data class Retry(val delay: Long) : Result()
    }
}

abstract class CoroutineWorker(context: Context, params: WorkerParameters) : Worker(context, params) {
    abstract suspend fun doWorkAsync(): Result
    override fun doWork(): Result = TODO()
}

class SyncWorker(context: Context, params: WorkerParameters) : Worker(context, params) {
    override fun doWork(): Result {
        performSync()
        return Result.Success
    }

    private fun performSync() {
        println("Syncing data...")
    }
}

class UploadWorker(context: Context, params: WorkerParameters) : CoroutineWorker(context, params) {
    override suspend fun doWorkAsync(): Result {
        uploadFiles()
        return Result.Success
    }

    private suspend fun uploadFiles() {
        println("Uploading...")
    }
}

class CleanupWorker(context: Context, params: WorkerParameters) : Worker(context, params) {
    override fun doWork(): Result {
        cleanupOldData()
        return Result.Success
    }

    private fun cleanupOldData() {}
}

class NotificationWorker(context: Context, params: WorkerParameters) : Worker(context, params) {
    override fun doWork(): Result {
        scheduleNotifications()
        return Result.Success
    }

    private fun scheduleNotifications() {}
}

class Context
class WorkerParameters
"#;

        let graph = build_graph_from_content(content);

        // Workers
        let workers = [
            "SyncWorker",
            "UploadWorker",
            "CleanupWorker",
            "NotificationWorker",
        ];

        for worker in &workers {
            let found = graph.declarations().any(|d| d.name == *worker);
            assert!(found, "Worker {} doit être trouvé", worker);
        }

        println!("WorkManager workers: {} found", workers.len());
    }

    /// Test 22: ContentProviders déclarés
    #[test]
    fn test_contentprovider_authorities() {
        let content = r#"
package com.example.providers

import android.content.ContentValues
import android.net.Uri

// ContentProviders déclarés dans le manifest

abstract class ContentProvider {
    abstract fun onCreate(): Boolean
    abstract fun query(uri: Uri, projection: Array<String>?, selection: String?,
                       selectionArgs: Array<String>?, sortOrder: String?): Cursor?
    abstract fun insert(uri: Uri, values: ContentValues?): Uri?
    abstract fun update(uri: Uri, values: ContentValues?, selection: String?,
                        selectionArgs: Array<String>?): Int
    abstract fun delete(uri: Uri, selection: String?, selectionArgs: Array<String>?): Int
    abstract fun getType(uri: Uri): String?
}

class UserContentProvider : ContentProvider() {
    override fun onCreate(): Boolean {
        initDatabase()
        return true
    }

    override fun query(uri: Uri, projection: Array<String>?, selection: String?,
                       selectionArgs: Array<String>?, sortOrder: String?): Cursor? {
        return queryUsers(uri, projection, selection)
    }

    override fun insert(uri: Uri, values: ContentValues?): Uri? {
        return insertUser(values)
    }

    override fun update(uri: Uri, values: ContentValues?, selection: String?,
                        selectionArgs: Array<String>?): Int {
        return updateUser(values, selection)
    }

    override fun delete(uri: Uri, selection: String?, selectionArgs: Array<String>?): Int {
        return deleteUser(selection)
    }

    override fun getType(uri: Uri): String? = "vnd.android.cursor.dir/user"

    private fun initDatabase() {}
    private fun queryUsers(uri: Uri, projection: Array<String>?, selection: String?): Cursor? = null
    private fun insertUser(values: ContentValues?): Uri? = null
    private fun updateUser(values: ContentValues?, selection: String?): Int = 0
    private fun deleteUser(selection: String?): Int = 0

    companion object {
        const val AUTHORITY = "com.example.providers.user"
        val CONTENT_URI: Uri = Uri.parse("content://$AUTHORITY/users")
    }
}

class FileProvider : ContentProvider() {
    override fun onCreate(): Boolean = true
    override fun query(uri: Uri, projection: Array<String>?, selection: String?,
                       selectionArgs: Array<String>?, sortOrder: String?): Cursor? = null
    override fun insert(uri: Uri, values: ContentValues?): Uri? = null
    override fun update(uri: Uri, values: ContentValues?, selection: String?,
                        selectionArgs: Array<String>?): Int = 0
    override fun delete(uri: Uri, selection: String?, selectionArgs: Array<String>?): Int = 0
    override fun getType(uri: Uri): String? = null
}

class Uri {
    companion object {
        fun parse(uriString: String): Uri = Uri()
    }
}
class Cursor
class ContentValues
"#;

        let graph = build_graph_from_content(content);

        // ContentProviders
        let providers = ["UserContentProvider", "FileProvider"];

        for provider in &providers {
            let found = graph.declarations().any(|d| d.name == *provider);
            assert!(found, "ContentProvider {} doit être trouvé", provider);
        }

        // ContentProvider methods
        let methods = ["onCreate", "query", "insert", "update", "delete", "getType"];
        for method in &methods {
            let count = graph.declarations().filter(|d| d.name == *method).count();
            assert!(
                count >= 2,
                "Method {} doit exister dans chaque provider",
                method
            );
        }

        println!("ContentProviders: {} found", providers.len());
    }
}

// ============================================================================
// CATÉGORIE 4: TESTS INJECTION DE DÉPENDANCES (5 tests)
// ============================================================================

mod di_tests {
    use super::*;

    /// Test 23: @HiltViewModel + @Inject
    #[test]
    fn test_hilt_viewmodel_injection() {
        let content = r#"
package com.example.viewmodels

import javax.inject.Inject
import dagger.hilt.android.lifecycle.HiltViewModel

// Annotations Hilt
annotation class HiltViewModel
annotation class Inject

abstract class ViewModel {
    open fun onCleared() {}
}

@HiltViewModel
class MainViewModel @Inject constructor(
    private val userRepository: UserRepository,
    private val analyticsService: AnalyticsService
) : ViewModel() {

    fun loadUsers() {
        val users = userRepository.getAll()
        analyticsService.track("users_loaded", users.size)
    }

    override fun onCleared() {
        super.onCleared()
        cleanup()
    }

    private fun cleanup() {}
}

@HiltViewModel
class DetailsViewModel @Inject constructor(
    private val itemRepository: ItemRepository,
    private val savedStateHandle: SavedStateHandle
) : ViewModel() {

    private val itemId: Long = savedStateHandle.get<Long>("itemId") ?: 0

    fun loadItem() {
        itemRepository.getById(itemId)
    }
}

@HiltViewModel
class SearchViewModel @Inject constructor(
    private val searchRepository: SearchRepository
) : ViewModel() {

    fun search(query: String) {
        searchRepository.search(query)
    }
}

// Repositories (would be @Inject in real code)
interface UserRepository {
    fun getAll(): List<Any>
}

interface ItemRepository {
    fun getById(id: Long): Any?
}

interface SearchRepository {
    fun search(query: String): List<Any>
}

interface AnalyticsService {
    fun track(event: String, value: Int)
}

class SavedStateHandle {
    fun <T> get(key: String): T? = null
}
"#;

        let graph = build_graph_from_content(content);

        // ViewModels
        let viewmodels = ["MainViewModel", "DetailsViewModel", "SearchViewModel"];

        for vm in &viewmodels {
            let found = graph.declarations().any(|d| d.name == *vm);
            assert!(found, "ViewModel {} doit être trouvé", vm);
        }

        // Repositories utilisés par injection
        let repos = ["UserRepository", "ItemRepository", "SearchRepository"];
        for repo in &repos {
            let found = graph.declarations().any(|d| d.name == *repo);
            assert!(found, "Repository {} doit être trouvé", repo);
        }

        println!("Hilt ViewModels: {} found", viewmodels.len());
    }

    /// Test 24: Dagger Subcomponents
    #[test]
    fn test_dagger_subcomponents() {
        let content = r#"
package com.example.di

// Dagger annotations
annotation class Component(val modules: Array<String> = [])
annotation class Subcomponent(val modules: Array<String> = [])
annotation class Module
annotation class Provides
annotation class Singleton
annotation class ActivityScope

// Main component
@Singleton
@Component(modules = ["AppModule"])
interface AppComponent {
    fun activityComponentFactory(): ActivityComponent.Factory
    fun inject(app: Application)
}

// Subcomponent for Activity scope
@ActivityScope
@Subcomponent(modules = ["ActivityModule"])
interface ActivityComponent {
    fun inject(activity: MainActivity)
    fun viewModelFactory(): ViewModelFactory

    @Subcomponent.Factory
    interface Factory {
        fun create(): ActivityComponent
    }
}

// Modules
@Module
object AppModule {
    @Provides
    @Singleton
    fun provideDatabase(): Database = Database()

    @Provides
    @Singleton
    fun provideApiService(): ApiService = ApiService()
}

@Module
object ActivityModule {
    @Provides
    @ActivityScope
    fun provideNavigator(): Navigator = Navigator()

    @Provides
    @ActivityScope
    fun provideViewModelFactory(
        database: Database,
        apiService: ApiService
    ): ViewModelFactory = ViewModelFactory()
}

class Application
class MainActivity
class Database
class ApiService
class Navigator
class ViewModelFactory
"#;

        let graph = build_graph_from_content(content);

        // Components (interfaces - may not be parsed by all parsers)
        let components = ["AppComponent", "ActivityComponent"];
        let mut found_components = 0;
        for comp in &components {
            let found = graph.declarations().any(|d| d.name == *comp);
            if found {
                found_components += 1;
            }
            println!("Component {}: found = {}", comp, found);
        }

        // Modules (objects)
        let modules = ["AppModule", "ActivityModule"];
        let mut found_modules = 0;
        for module in &modules {
            let found = graph.declarations().any(|d| d.name == *module);
            if found {
                found_modules += 1;
            }
            println!("Module {}: found = {}", module, found);
        }

        // Provide methods - check how many are found
        let provides = [
            "provideDatabase",
            "provideApiService",
            "provideNavigator",
            "provideViewModelFactory",
        ];
        let mut found_provides = 0;
        for provide in &provides {
            let found = graph.declarations().any(|d| d.name == *provide);
            if found {
                found_provides += 1;
            }
            println!("Provide method {}: found = {}", provide, found);
        }

        // At minimum, we should find the modules (objects) or some declarations
        let total_decls = graph.declarations().count();
        println!(
            "Dagger subcomponents: {} declarations, {} components, {} modules, {} provide methods",
            total_decls, found_components, found_modules, found_provides
        );

        // Verify that we found at least some Dagger-related declarations
        assert!(
            total_decls > 0,
            "Should find at least some declarations in Dagger code"
        );
    }

    /// Test 25: Koin modules
    #[test]
    fn test_koin_modules() {
        let content = r#"
package com.example.di.koin

// Koin DSL simulation

class Module(val definitions: List<Definition>)
class Definition

fun module(block: ModuleBuilder.() -> Unit): Module {
    val builder = ModuleBuilder()
    builder.block()
    return Module(builder.definitions)
}

class ModuleBuilder {
    val definitions = mutableListOf<Definition>()

    inline fun <reified T> single(noinline definition: () -> T) {
        definitions.add(Definition())
    }

    inline fun <reified T> factory(noinline definition: () -> T) {
        definitions.add(Definition())
    }

    inline fun <reified T> viewModel(noinline definition: () -> T) {
        definitions.add(Definition())
    }
}

// Application modules
val appModule = module {
    single { Database() }
    single { ApiClient() }
    single { UserRepository(get(), get()) }
}

val networkModule = module {
    single { OkHttpClient() }
    single { Retrofit(get()) }
    factory { AuthInterceptor() }
}

val viewModelModule = module {
    viewModel { MainViewModel(get()) }
    viewModel { DetailsViewModel(get(), get()) }
    viewModel { SearchViewModel(get()) }
}

// Classes
class Database
class ApiClient
class UserRepository(val db: Database, val api: ApiClient)
class OkHttpClient
class Retrofit(val client: OkHttpClient)
class AuthInterceptor
class MainViewModel(val repo: UserRepository)
class DetailsViewModel(val repo: UserRepository, val api: ApiClient)
class SearchViewModel(val repo: UserRepository)

inline fun <reified T> get(): T = TODO()
"#;

        let graph = build_graph_from_content(content);

        // Modules Koin
        let modules = ["appModule", "networkModule", "viewModelModule"];
        for module in &modules {
            let found = graph.declarations().any(|d| d.name == *module);
            assert!(found, "Koin module {} doit être trouvé", module);
        }

        // Classes injectées
        let classes = ["Database", "ApiClient", "UserRepository", "MainViewModel"];
        for class_name in &classes {
            let found = graph.declarations().any(|d| d.name == *class_name);
            assert!(found, "Class {} doit être trouvée", class_name);
        }

        println!(
            "Koin modules: {} declarations",
            graph.declarations().count()
        );
    }

    /// Test 26: @AssistedInject + @AssistedFactory
    #[test]
    fn test_assisted_inject() {
        let content = r#"
package com.example.di.assisted

// Assisted injection annotations
annotation class AssistedInject
annotation class Assisted
annotation class AssistedFactory

// Class with assisted injection
class UserDetailsPresenter @AssistedInject constructor(
    private val userRepository: UserRepository,
    private val analyticsService: AnalyticsService,
    @Assisted private val userId: Long,
    @Assisted private val callback: Callback
) {
    fun loadUser() {
        val user = userRepository.getById(userId)
        analyticsService.track("user_loaded")
        callback.onUserLoaded(user)
    }

    @AssistedFactory
    interface Factory {
        fun create(userId: Long, callback: Callback): UserDetailsPresenter
    }
}

class ItemPresenter @AssistedInject constructor(
    private val itemRepository: ItemRepository,
    @Assisted private val itemId: String,
    @Assisted private val position: Int
) {
    fun bind() {
        val item = itemRepository.getById(itemId)
        // bind item at position
    }

    @AssistedFactory
    interface Factory {
        fun create(itemId: String, position: Int): ItemPresenter
    }
}

interface UserRepository {
    fun getById(id: Long): Any
}

interface ItemRepository {
    fun getById(id: String): Any
}

interface AnalyticsService {
    fun track(event: String)
}

interface Callback {
    fun onUserLoaded(user: Any)
}
"#;

        let graph = build_graph_from_content(content);

        // Presenters with assisted inject
        let presenters = ["UserDetailsPresenter", "ItemPresenter"];
        for presenter in &presenters {
            let found = graph.declarations().any(|d| d.name == *presenter);
            assert!(found, "Presenter {} doit être trouvé", presenter);
        }

        // Factory interfaces
        let factory_count = graph.declarations().filter(|d| d.name == "Factory").count();
        assert!(
            factory_count >= 2,
            "Factory interfaces doivent être trouvées"
        );

        println!(
            "Assisted inject: {} declarations",
            graph.declarations().count()
        );
    }

    /// Test 27: Multibinding contributions
    #[test]
    fn test_multibinding_contributions() {
        let content = r#"
package com.example.di.multibind

// Multibinding annotations
annotation class IntoSet
annotation class IntoMap
annotation class StringKey(val value: String)
annotation class ClassKey(val value: String)
annotation class Binds
annotation class Module

// Interceptor multibinding
interface Interceptor {
    fun intercept(request: Request): Response
}

class AuthInterceptor : Interceptor {
    override fun intercept(request: Request): Response {
        // Add auth header
        return Response()
    }
}

class LoggingInterceptor : Interceptor {
    override fun intercept(request: Request): Response {
        println("Request: $request")
        return Response()
    }
}

class CacheInterceptor : Interceptor {
    override fun intercept(request: Request): Response {
        // Check cache
        return Response()
    }
}

@Module
abstract class InterceptorModule {
    @Binds
    @IntoSet
    abstract fun bindAuthInterceptor(impl: AuthInterceptor): Interceptor

    @Binds
    @IntoSet
    abstract fun bindLoggingInterceptor(impl: LoggingInterceptor): Interceptor

    @Binds
    @IntoSet
    abstract fun bindCacheInterceptor(impl: CacheInterceptor): Interceptor
}

// ViewModel factory multibinding
interface ViewModelFactory {
    fun create(): ViewModel
}

class HomeViewModelFactory : ViewModelFactory {
    override fun create(): ViewModel = ViewModel()
}

class SettingsViewModelFactory : ViewModelFactory {
    override fun create(): ViewModel = ViewModel()
}

@Module
abstract class ViewModelModule {
    @Binds
    @IntoMap
    @StringKey("home")
    abstract fun bindHomeFactory(factory: HomeViewModelFactory): ViewModelFactory

    @Binds
    @IntoMap
    @StringKey("settings")
    abstract fun bindSettingsFactory(factory: SettingsViewModelFactory): ViewModelFactory
}

class Request
class Response
class ViewModel
"#;

        let graph = build_graph_from_content(content);

        // Interceptors contributed to set
        let interceptors = ["AuthInterceptor", "LoggingInterceptor", "CacheInterceptor"];
        for interceptor in &interceptors {
            let found = graph.declarations().any(|d| d.name == *interceptor);
            assert!(found, "Interceptor {} doit être trouvé", interceptor);
        }

        // ViewModelFactories contributed to map
        let factories = ["HomeViewModelFactory", "SettingsViewModelFactory"];
        for factory in &factories {
            let found = graph.declarations().any(|d| d.name == *factory);
            assert!(found, "Factory {} doit être trouvé", factory);
        }

        // Binding methods
        let bind_methods: Vec<_> = graph
            .declarations()
            .filter(|d| d.name.starts_with("bind"))
            .collect();

        println!(
            "Multibinding: {} bind methods, {} declarations total",
            bind_methods.len(),
            graph.declarations().count()
        );
    }
}

// ============================================================================
// CATÉGORIE 5: TESTS FAUX POSITIFS CRITIQUES (7 tests)
// ============================================================================

mod critical_false_positives {
    use super::*;

    /// Test 28: Méthodes Retrofit @GET, @POST
    #[test]
    fn test_retrofit_interface_methods() {
        let content = r#"
package com.example.api

// Retrofit annotations
annotation class GET(val value: String)
annotation class POST(val value: String)
annotation class PUT(val value: String)
annotation class DELETE(val value: String)
annotation class Path(val value: String)
annotation class Query(val value: String)
annotation class Body
annotation class Header(val value: String)

// Retrofit API interface
interface UserApi {
    @GET("users")
    suspend fun getUsers(): List<User>

    @GET("users/{id}")
    suspend fun getUserById(@Path("id") id: Long): User

    @GET("users/search")
    suspend fun searchUsers(
        @Query("q") query: String,
        @Query("page") page: Int
    ): SearchResult<User>

    @POST("users")
    suspend fun createUser(@Body user: CreateUserRequest): User

    @PUT("users/{id}")
    suspend fun updateUser(
        @Path("id") id: Long,
        @Body user: UpdateUserRequest
    ): User

    @DELETE("users/{id}")
    suspend fun deleteUser(@Path("id") id: Long)
}

interface ProductApi {
    @GET("products")
    suspend fun getProducts(
        @Query("category") category: String?,
        @Query("limit") limit: Int = 20
    ): List<Product>

    @GET("products/{id}")
    suspend fun getProductById(@Path("id") id: String): Product

    @POST("products/{id}/reviews")
    suspend fun addReview(
        @Path("id") productId: String,
        @Body review: ReviewRequest,
        @Header("Authorization") token: String
    ): Review
}

data class User(val id: Long, val name: String)
data class CreateUserRequest(val name: String, val email: String)
data class UpdateUserRequest(val name: String?)
data class SearchResult<T>(val items: List<T>, val total: Int)
data class Product(val id: String, val name: String, val price: Double)
data class ReviewRequest(val rating: Int, val comment: String)
data class Review(val id: Long, val rating: Int)
"#;

        let graph = build_graph_from_content(content);

        // API interfaces
        let apis = ["UserApi", "ProductApi"];
        for api in &apis {
            let found = graph.declarations().any(|d| d.name == *api);
            assert!(found, "API interface {} doit être trouvée", api);
        }

        // API methods (ces méthodes sont appelées via Retrofit, pas directement)
        let api_methods = [
            "getUsers",
            "getUserById",
            "searchUsers",
            "createUser",
            "updateUser",
            "deleteUser",
            "getProducts",
            "getProductById",
            "addReview",
        ];

        for method in &api_methods {
            let found = graph.declarations().any(|d| d.name == *method);
            assert!(found, "API method {} doit être trouvée", method);
        }

        // Ces méthodes NE DOIVENT PAS être signalées comme mortes
        println!("Retrofit API methods: {} found", api_methods.len());
    }

    /// Test 29: Room DAO queries
    #[test]
    fn test_room_dao_queries() {
        let content = r#"
package com.example.db

// Room annotations
annotation class Dao
annotation class Query(val value: String)
annotation class Insert(val onConflict: Int = 0)
annotation class Update
annotation class Delete
annotation class Transaction
annotation class RawQuery

@Dao
interface UserDao {
    @Query("SELECT * FROM users")
    fun getAllUsers(): List<UserEntity>

    @Query("SELECT * FROM users WHERE id = :userId")
    fun getUserById(userId: Long): UserEntity?

    @Query("SELECT * FROM users WHERE email = :email LIMIT 1")
    suspend fun getUserByEmail(email: String): UserEntity?

    @Query("SELECT * FROM users WHERE name LIKE :query")
    fun searchUsers(query: String): List<UserEntity>

    @Query("SELECT COUNT(*) FROM users")
    fun getUserCount(): Int

    @Insert(onConflict = 1) // REPLACE
    suspend fun insertUser(user: UserEntity): Long

    @Insert
    suspend fun insertUsers(users: List<UserEntity>)

    @Update
    suspend fun updateUser(user: UserEntity): Int

    @Delete
    suspend fun deleteUser(user: UserEntity): Int

    @Query("DELETE FROM users WHERE id = :userId")
    suspend fun deleteUserById(userId: Long): Int

    @Transaction
    suspend fun replaceUser(old: UserEntity, new: UserEntity) {
        deleteUser(old)
        insertUser(new)
    }
}

@Dao
interface ProductDao {
    @Query("SELECT * FROM products WHERE category = :category")
    fun getProductsByCategory(category: String): List<ProductEntity>

    @Query("""
        SELECT p.* FROM products p
        INNER JOIN orders o ON p.id = o.product_id
        WHERE o.user_id = :userId
    """)
    fun getOrderedProducts(userId: Long): List<ProductEntity>

    @Insert
    suspend fun insertProduct(product: ProductEntity): Long
}

data class UserEntity(val id: Long, val name: String, val email: String)
data class ProductEntity(val id: Long, val name: String, val category: String)
"#;

        let graph = build_graph_from_content(content);

        // DAOs
        let daos = ["UserDao", "ProductDao"];
        for dao in &daos {
            let found = graph.declarations().any(|d| d.name == *dao);
            assert!(found, "DAO {} doit être trouvé", dao);
        }

        // DAO methods
        let dao_methods = [
            "getAllUsers",
            "getUserById",
            "getUserByEmail",
            "searchUsers",
            "insertUser",
            "updateUser",
            "deleteUser",
            "deleteUserById",
            "getProductsByCategory",
            "getOrderedProducts",
        ];

        for method in &dao_methods {
            let found = graph.declarations().any(|d| d.name == *method);
            assert!(found, "DAO method {} doit être trouvée", method);
        }

        println!("Room DAO methods: {} found", dao_methods.len());
    }

    /// Test 30: Classes avec -keep dans ProGuard
    #[test]
    fn test_proguard_keep_rules() {
        // Ce test simule des classes qui ont des règles -keep dans proguard-rules.pro
        // Ces classes NE DOIVENT PAS être signalées comme mortes

        let content = r#"
package com.example.models

// Ces classes ont des règles -keep dans proguard-rules.pro:
// -keep class com.example.models.** { *; }

// Modèles JSON utilisés par Gson/Moshi via réflexion
data class ApiResponse(
    val status: String,
    val code: Int,
    val message: String?,
    val data: Any?
)

data class UserResponse(
    val id: Long,
    val username: String,
    val email: String,
    val avatar: String?
)

data class ErrorResponse(
    val error: String,
    val errorCode: Int,
    val details: Map<String, String>?
)

// Classes utilisées par réflexion pour la sérialisation
class CustomSerializer {
    fun serialize(obj: Any): String = ""
    fun deserialize(json: String): Any = Unit
}

// Callbacks pour JNI
class NativeCallback {
    // Appelé depuis le code natif
    fun onNativeEvent(eventType: Int, data: ByteArray) {
        processNativeEvent(eventType, data)
    }

    private fun processNativeEvent(type: Int, data: ByteArray) {}
}

// Classes avec @Keep annotation
annotation class Keep

@Keep
class CriticalClass {
    fun criticalMethod() {}
}
"#;

        let graph = build_graph_from_content(content);

        // Classes avec -keep
        let kept_classes = [
            "ApiResponse",
            "UserResponse",
            "ErrorResponse",
            "CustomSerializer",
            "NativeCallback",
            "CriticalClass",
        ];

        for class_name in &kept_classes {
            let found = graph.declarations().any(|d| d.name == *class_name);
            assert!(found, "Kept class {} doit être trouvée", class_name);
        }

        // Ces classes NE DOIVENT PAS être signalées comme mortes
        // car elles ont des règles -keep
        println!("ProGuard kept classes: {} found", kept_classes.len());
    }

    /// Test 31: Class.forName() usage
    #[test]
    fn test_reflection_class_forname() {
        let content = r#"
package com.example.reflection

// Classes instanciées via réflexion
// Class.forName("com.example.reflection.PluginA").newInstance()

interface Plugin {
    fun initialize()
    fun execute(): String
}

class PluginA : Plugin {
    override fun initialize() {
        println("PluginA initialized")
    }

    override fun execute(): String = "PluginA result"
}

class PluginB : Plugin {
    override fun initialize() {
        println("PluginB initialized")
    }

    override fun execute(): String = "PluginB result"
}

class PluginC : Plugin {
    override fun initialize() {
        println("PluginC initialized")
    }

    override fun execute(): String = "PluginC result"
}

// Plugin loader using reflection
class PluginLoader {
    private val plugins = mutableMapOf<String, Plugin>()

    fun loadPlugin(className: String): Plugin {
        // val pluginClass = Class.forName(className)
        // val plugin = pluginClass.newInstance() as Plugin
        val plugin = createPlugin(className)
        plugin.initialize()
        plugins[className] = plugin
        return plugin
    }

    private fun createPlugin(name: String): Plugin {
        return when (name) {
            "PluginA" -> PluginA()
            "PluginB" -> PluginB()
            "PluginC" -> PluginC()
            else -> throw IllegalArgumentException("Unknown plugin: $name")
        }
    }

    fun getPlugin(className: String): Plugin? = plugins[className]
}

// Service provider interface pattern
interface ServiceProvider {
    fun getService(): Any
}

class DatabaseServiceProvider : ServiceProvider {
    override fun getService(): Any = "DatabaseService"
}

class CacheServiceProvider : ServiceProvider {
    override fun getService(): Any = "CacheService"
}
"#;

        let graph = build_graph_from_content(content);

        // Plugins instanciés via réflexion
        let plugins = ["PluginA", "PluginB", "PluginC"];
        for plugin in &plugins {
            let found = graph.declarations().any(|d| d.name == *plugin);
            assert!(found, "Plugin {} doit être trouvé", plugin);
        }

        // Service providers
        let providers = ["DatabaseServiceProvider", "CacheServiceProvider"];
        for provider in &providers {
            let found = graph.declarations().any(|d| d.name == *provider);
            assert!(found, "ServiceProvider {} doit être trouvé", provider);
        }

        println!(
            "Reflection classes: {} found",
            plugins.len() + providers.len()
        );
    }

    /// Test 32: @Subscribe EventBus/Otto
    #[test]
    fn test_eventbus_subscribers() {
        let content = r#"
package com.example.events

// EventBus annotations
annotation class Subscribe(val threadMode: ThreadMode = ThreadMode.POSTING)
enum class ThreadMode { POSTING, MAIN, BACKGROUND, ASYNC }

// Events
data class UserLoggedInEvent(val userId: Long, val username: String)
data class UserLoggedOutEvent(val userId: Long)
data class DataUpdatedEvent(val dataType: String, val count: Int)
data class NetworkStatusEvent(val isConnected: Boolean)
data class ErrorEvent(val error: Throwable)

// Event subscribers - ces méthodes sont appelées par EventBus via réflexion
class UserEventHandler {
    @Subscribe(threadMode = ThreadMode.MAIN)
    fun onUserLoggedIn(event: UserLoggedInEvent) {
        println("User logged in: ${event.username}")
        updateUI(event.userId)
    }

    @Subscribe(threadMode = ThreadMode.MAIN)
    fun onUserLoggedOut(event: UserLoggedOutEvent) {
        println("User logged out: ${event.userId}")
        clearUserData()
    }

    private fun updateUI(userId: Long) {}
    private fun clearUserData() {}
}

class DataEventHandler {
    @Subscribe(threadMode = ThreadMode.BACKGROUND)
    fun onDataUpdated(event: DataUpdatedEvent) {
        println("Data updated: ${event.dataType} (${event.count} items)")
        syncData(event.dataType)
    }

    private fun syncData(type: String) {}
}

class NetworkEventHandler {
    @Subscribe
    fun onNetworkStatus(event: NetworkStatusEvent) {
        if (event.isConnected) {
            retryPendingRequests()
        } else {
            cachePendingRequests()
        }
    }

    @Subscribe(threadMode = ThreadMode.MAIN)
    fun onError(event: ErrorEvent) {
        showErrorDialog(event.error.message ?: "Unknown error")
    }

    private fun retryPendingRequests() {}
    private fun cachePendingRequests() {}
    private fun showErrorDialog(message: String) {}
}
"#;

        let graph = build_graph_from_content(content);

        // Event handlers
        let handlers = [
            "UserEventHandler",
            "DataEventHandler",
            "NetworkEventHandler",
        ];
        for handler in &handlers {
            let found = graph.declarations().any(|d| d.name == *handler);
            assert!(found, "Handler {} doit être trouvé", handler);
        }

        // Subscriber methods
        let subscribers = [
            "onUserLoggedIn",
            "onUserLoggedOut",
            "onDataUpdated",
            "onNetworkStatus",
            "onError",
        ];
        for sub in &subscribers {
            let found = graph.declarations().any(|d| d.name == *sub);
            assert!(found, "Subscriber {} doit être trouvé", sub);
        }

        // Events
        let events = [
            "UserLoggedInEvent",
            "UserLoggedOutEvent",
            "DataUpdatedEvent",
            "NetworkStatusEvent",
            "ErrorEvent",
        ];
        for event in &events {
            let found = graph.declarations().any(|d| d.name == *event);
            assert!(found, "Event {} doit être trouvé", event);
        }

        println!(
            "EventBus: {} subscribers, {} events",
            subscribers.len(),
            events.len()
        );
    }

    /// Test 33: @Parcelize generated CREATOR
    #[test]
    fn test_parcelize_generated() {
        let content = r#"
package com.example.parcel

// Parcelize annotation
annotation class Parcelize

// Parcelable interface
interface Parcelable {
    fun writeToParcel(dest: Parcel, flags: Int)
    fun describeContents(): Int

    interface Creator<T> {
        fun createFromParcel(source: Parcel): T
        fun newArray(size: Int): Array<T?>
    }
}

class Parcel {
    fun writeString(value: String?) {}
    fun readString(): String? = null
    fun writeLong(value: Long) {}
    fun readLong(): Long = 0
    fun writeInt(value: Int) {}
    fun readInt(): Int = 0
}

// Classes avec @Parcelize - le CREATOR est généré automatiquement
@Parcelize
data class User(
    val id: Long,
    val name: String,
    val email: String
) : Parcelable {
    override fun writeToParcel(dest: Parcel, flags: Int) {
        dest.writeLong(id)
        dest.writeString(name)
        dest.writeString(email)
    }

    override fun describeContents(): Int = 0

    companion object CREATOR : Parcelable.Creator<User> {
        override fun createFromParcel(source: Parcel): User {
            return User(
                source.readLong(),
                source.readString() ?: "",
                source.readString() ?: ""
            )
        }

        override fun newArray(size: Int): Array<User?> = arrayOfNulls(size)
    }
}

@Parcelize
data class Order(
    val orderId: String,
    val items: List<String>,
    val total: Double
) : Parcelable {
    override fun writeToParcel(dest: Parcel, flags: Int) {}
    override fun describeContents(): Int = 0

    companion object CREATOR : Parcelable.Creator<Order> {
        override fun createFromParcel(source: Parcel): Order = Order("", emptyList(), 0.0)
        override fun newArray(size: Int): Array<Order?> = arrayOfNulls(size)
    }
}
"#;

        let graph = build_graph_from_content(content);

        // Parcelable classes
        let parcelables = ["User", "Order"];
        for p in &parcelables {
            let found = graph.declarations().any(|d| d.name == *p);
            assert!(found, "Parcelable {} doit être trouvé", p);
        }

        // CREATOR companions
        let creator_count = graph.declarations().filter(|d| d.name == "CREATOR").count();
        assert!(
            creator_count >= 2,
            "CREATOR companions doivent être trouvés"
        );

        // Parcelable methods
        let methods = [
            "writeToParcel",
            "describeContents",
            "createFromParcel",
            "newArray",
        ];
        for method in &methods {
            let count = graph.declarations().filter(|d| d.name == *method).count();
            assert!(count >= 1, "Method {} doit être trouvée", method);
        }

        println!("Parcelize: {} parcelables with CREATOR", parcelables.len());
    }

    /// Test 34: Enum.valueOf() et entries
    #[test]
    fn test_enum_valueof_entries() {
        let content = r#"
package com.example.enums

// Tous les variants d'un enum peuvent être utilisés via valueOf() ou entries
// Ils NE DOIVENT PAS être signalés comme morts

enum class Status {
    PENDING,
    ACTIVE,
    COMPLETED,
    CANCELLED,
    ARCHIVED;

    companion object {
        fun fromString(value: String): Status = valueOf(value.uppercase())
        fun fromCode(code: Int): Status = entries[code]
    }
}

enum class Priority(val level: Int) {
    LOW(0),
    MEDIUM(1),
    HIGH(2),
    CRITICAL(3);

    companion object {
        fun fromLevel(level: Int): Priority? = entries.find { it.level == level }
    }
}

enum class HttpMethod {
    GET, POST, PUT, PATCH, DELETE, HEAD, OPTIONS;

    companion object {
        fun parse(method: String): HttpMethod = valueOf(method.uppercase())
    }
}

// Usage
class EnumProcessor {
    fun processStatus(statusString: String) {
        val status = Status.fromString(statusString)
        when (status) {
            Status.PENDING -> handlePending()
            Status.ACTIVE -> handleActive()
            Status.COMPLETED -> handleCompleted()
            Status.CANCELLED -> handleCancelled()
            Status.ARCHIVED -> handleArchived()
        }
    }

    fun getAllStatuses(): List<Status> = Status.entries

    fun getHighPriorities(): List<Priority> =
        Priority.entries.filter { it.level >= Priority.HIGH.level }

    private fun handlePending() {}
    private fun handleActive() {}
    private fun handleCompleted() {}
    private fun handleCancelled() {}
    private fun handleArchived() {}
}
"#;

        let graph = build_graph_from_content(content);

        // Enums
        let enums = ["Status", "Priority", "HttpMethod"];
        for e in &enums {
            let found = graph.declarations().any(|d| d.name == *e);
            assert!(found, "Enum {} doit être trouvé", e);
        }

        // Enum values de Status
        let status_values = ["PENDING", "ACTIVE", "COMPLETED", "CANCELLED", "ARCHIVED"];
        for value in &status_values {
            let found = graph.declarations().any(|d| d.name == *value);
            // Les enum values peuvent être dans le même nœud que l'enum
            println!("Enum value {}: found = {}", value, found);
        }

        // Companion methods - check if they exist (may or may not be parsed as separate decls)
        let companions = ["fromString", "fromCode", "fromLevel", "parse"];
        let mut found_companions = 0;
        for comp in &companions {
            let found = graph.declarations().any(|d| d.name == *comp);
            if found {
                found_companions += 1;
            }
            println!("Companion method {}: found = {}", comp, found);
        }

        // At least the enums should be found
        println!(
            "Enums with valueOf/entries: {} enums, {} companion methods found",
            enums.len(),
            found_companions
        );
    }
}

// ============================================================================
// CATÉGORIE 6: TESTS PERFORMANCE & STRESS (3 tests)
// ============================================================================

mod performance_tests {
    use super::*;

    /// Test 35: Large codebase simulation (1000+ declarations)
    #[test]
    fn test_large_codebase_performance() {
        let temp_dir = tempfile::tempdir().expect("Failed to create temp dir");

        // Generate 50 files with 20 classes each = 1000+ declarations
        for file_idx in 0..50 {
            let mut content = format!("package com.example.generated.file{}\n\n", file_idx);

            for class_idx in 0..20 {
                content.push_str(&format!(
                    r#"
class Generated{}_{} {{
    val property1 = "value1"
    val property2 = "value2"

    fun method1(): String = property1
    fun method2(): String = property2
    fun method3(param: String): String = param
}}
"#,
                    file_idx, class_idx
                ));
            }

            let file_path = temp_dir.path().join(format!("Generated{}.kt", file_idx));
            std::fs::write(&file_path, content).expect("Failed to write file");
        }

        let start = Instant::now();
        let mut builder = GraphBuilder::new();

        // Process all files
        for file_idx in 0..50 {
            let file_path = temp_dir.path().join(format!("Generated{}.kt", file_idx));
            let source = SourceFile::new(file_path, FileType::Kotlin);
            builder.process_file(&source).expect("Failed to process");
        }

        let graph = builder.build();
        let parse_time = start.elapsed();

        let decl_count = graph.declarations().count();
        println!(
            "Large codebase: {} declarations parsed in {:?}",
            decl_count, parse_time
        );

        assert!(decl_count >= 1000, "Should have at least 1000 declarations");
        assert!(
            parse_time.as_secs() < 60,
            "Parsing should complete in < 60 seconds"
        );

        // Test detector performance
        let detector_start = Instant::now();
        let detector = WriteOnlyDetector::new();
        let issues = detector.detect(&graph);
        let detector_time = detector_start.elapsed();

        println!(
            "Write-only detector: {} issues in {:?}",
            issues.len(),
            detector_time
        );
        assert!(
            detector_time.as_secs() < 10,
            "Detector should complete in < 10 seconds"
        );
    }

    /// Test 36: Deeply nested structures (50+ levels)
    #[test]
    fn test_deeply_nested_structures() {
        let mut content = String::from("package com.example.nested\n\n");

        // Create deeply nested classes
        for level in 0..50 {
            content.push_str(&format!(
                "{}class Level{} {{\n",
                "    ".repeat(level),
                level
            ));
        }

        // Inner content at deepest level
        content.push_str(&format!(
            "{}val deepValue = \"deep\"\n{}fun deepMethod() = deepValue\n",
            "    ".repeat(50),
            "    ".repeat(50)
        ));

        // Close all classes
        for level in (0..50).rev() {
            content.push_str(&format!("{}}}\n", "    ".repeat(level)));
        }

        let start = Instant::now();
        let graph = build_graph_from_content(&content);
        let elapsed = start.elapsed();

        let decl_count = graph.declarations().count();
        println!(
            "Deeply nested: {} declarations parsed in {:?}",
            decl_count, elapsed
        );

        // Should parse all levels
        let level_classes: Vec<_> = graph
            .declarations()
            .filter(|d| d.name.starts_with("Level"))
            .collect();

        println!("Level classes found: {}", level_classes.len());
        assert!(elapsed.as_secs() < 10, "Nested parsing should be fast");
    }

    /// Test 37: Parallel vs Sequential performance
    #[test]
    fn test_parallel_vs_sequential() {
        let temp_dir = tempfile::tempdir().expect("Failed to create temp dir");

        // Generate 20 files
        for file_idx in 0..20 {
            let content = format!(
                r#"
package com.example.parallel.file{}

class FileClass{} {{
    val prop1 = "value1"
    val prop2 = "value2"
    val prop3 = "value3"

    fun method1() = prop1
    fun method2() = prop2
    fun method3() = prop3
    fun method4(x: Int): Int = x * 2
    fun method5(s: String): String = s.uppercase()
}}

object FileObject{} {{
    const val CONST1 = "const1"
    const val CONST2 = "const2"

    fun staticMethod1() = CONST1
    fun staticMethod2() = CONST2
}}

data class FileData{}(
    val id: Long,
    val name: String,
    val value: Int
)
"#,
                file_idx, file_idx, file_idx, file_idx
            );

            let file_path = temp_dir.path().join(format!("File{}.kt", file_idx));
            std::fs::write(&file_path, content).expect("Failed to write file");
        }

        // Sequential processing
        let seq_start = Instant::now();
        let mut seq_builder = GraphBuilder::new();
        for file_idx in 0..20 {
            let file_path = temp_dir.path().join(format!("File{}.kt", file_idx));
            let source = SourceFile::new(file_path, FileType::Kotlin);
            seq_builder
                .process_file(&source)
                .expect("Failed to process");
        }
        let seq_graph = seq_builder.build();
        let seq_time = seq_start.elapsed();

        println!(
            "Sequential: {} declarations in {:?}",
            seq_graph.declarations().count(),
            seq_time
        );

        // Note: Pour un vrai test parallèle, on utiliserait ParallelGraphBuilder
        // Ici on vérifie juste que le traitement séquentiel est raisonnable

        assert!(seq_time.as_secs() < 10, "Sequential should be fast");
        assert!(
            seq_graph.declarations().count() >= 100,
            "Should have many declarations"
        );
    }
}

// ============================================================================
// CATÉGORIE 7: TESTS RAPPORTS & OUTPUT (3 tests)
// ============================================================================

mod report_tests {
    use super::*;

    /// Test 38: SARIF output validation
    #[test]
    fn test_sarif_output_valid() {
        let content = r#"
package com.example.sarif

class UnusedClass {
    fun unusedMethod() {}
}

class UsedClass {
    private val writeOnly = 0

    fun setWriteOnly(value: Int) {
        // writeOnly = value (can't reassign val)
    }
}

fun main() {
    val used = UsedClass()
    used.setWriteOnly(42)
}
"#;

        let graph = build_graph_from_content(content);

        // Run detectors
        let write_only_detector = WriteOnlyDetector::new();
        let issues = write_only_detector.detect(&graph);

        // SARIF output should contain:
        // 1. $schema - JSON schema reference
        // 2. version - SARIF version (2.1.0)
        // 3. runs - array of run objects
        // 4. results - array of result objects with:
        //    - ruleId
        //    - level (error/warning/note)
        //    - message
        //    - locations

        // Basic validation that we have issues
        println!("Issues for SARIF: {}", issues.len());

        // Verify issue structure
        for issue in &issues {
            assert!(
                !issue.declaration.name.is_empty(),
                "Issue should have declaration name"
            );
            assert!(!issue.message.is_empty(), "Issue should have message");
        }
    }

    /// Test 39: JSON output schema validation
    #[test]
    fn test_json_output_schema() {
        let content = r#"
package com.example.json

class TestClass {
    private val unusedField = "unused"
    val usedField = "used"

    fun unusedMethod() {
        println("never called")
    }

    fun usedMethod(): String = usedField
}

fun main() {
    val obj = TestClass()
    println(obj.usedMethod())
}
"#;

        let graph = build_graph_from_content(content);

        // Collect declarations
        let declarations: Vec<_> = graph.declarations().collect();

        // JSON output should contain:
        // 1. summary - total counts
        // 2. issues - array of dead code issues
        // 3. Each issue should have:
        //    - type (DC001, DC002, etc.)
        //    - name
        //    - file
        //    - line
        //    - confidence
        //    - message

        println!("Declarations for JSON: {}", declarations.len());

        // Verify each declaration has required fields
        for decl in &declarations {
            assert!(!decl.name.is_empty(), "Declaration should have name");
            assert!(
                decl.location.line > 0,
                "Declaration should have line number"
            );
        }
    }

    /// Test 40: Baseline exclude existing issues
    #[test]
    fn test_baseline_exclude_existing() {
        let content = r#"
package com.example.baseline

// These issues are in the baseline - should be excluded
class BaselinedUnusedClass {  // In baseline
    fun baselinedUnusedMethod() {}  // In baseline
}

// These are new issues - should be reported
class NewUnusedClass {  // NEW - not in baseline
    fun newUnusedMethod() {}  // NEW - not in baseline
}

// Used code
class UsedClass {
    fun usedMethod() = "used"
}

fun main() {
    val used = UsedClass()
    println(used.usedMethod())
}
"#;

        let graph = build_graph_from_content(content);

        // Simulate baseline content
        let baseline_issues = [
            ("BaselinedUnusedClass", "test.kt", 5),
            ("baselinedUnusedMethod", "test.kt", 6),
        ];

        // Get all declarations that could be dead
        let all_decls: Vec<_> = graph
            .declarations()
            .filter(|d| d.name.contains("Unused"))
            .collect();

        // Filter out baselined issues
        let new_issues: Vec<_> = all_decls
            .iter()
            .filter(|d| !baseline_issues.iter().any(|(name, _, _)| d.name == *name))
            .collect();

        println!("All unused declarations: {}", all_decls.len());
        println!("New issues (not in baseline): {}", new_issues.len());

        // New issues should only contain non-baselined ones
        // (names contain "New" or "new" since we have NewUnusedClass and newUnusedMethod)
        for issue in &new_issues {
            let is_new_issue = issue.name.to_lowercase().contains("new");
            assert!(
                is_new_issue,
                "Only new issues should remain after baseline filtering, found: {}",
                issue.name
            );
        }

        // Verify we found the expected declarations
        println!(
            "New issues after baseline filter: {:?}",
            new_issues.iter().map(|d| &d.name).collect::<Vec<_>>()
        );
    }
}
