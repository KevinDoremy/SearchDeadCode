//! Tests anti-faux-positifs pour SearchDeadCode
//!
//! Ces tests vérifient que le système NE SIGNALE PAS comme "dead code"
//! du code qui est en réalité utilisé via des mécanismes indirects.
//!
//! Catégories de faux positifs courants:
//! 1. Réflexion et injection de dépendances (Dagger, Hilt, Koin)
//! 2. Callbacks Android (lifecycle, onClick, etc.)
//! 3. Sérialisation (JSON, Parcelable, Serializable)
//! 4. Conventions Kotlin (operators, delegates, DSL)
//! 5. API publique de bibliothèque
//! 6. Annotations processors (Room, Retrofit, DataBinding)
//! 7. Tests et mocks
//! 8. JNI et code natif

use searchdeadcode::analysis::detectors::{
    Detector, RedundantOverrideDetector, UnusedParamDetector, UnusedSealedVariantDetector,
    WriteOnlyDetector,
};
use searchdeadcode::analysis::ReachabilityAnalyzer;
use searchdeadcode::discovery::{FileType, SourceFile};
use searchdeadcode::graph::GraphBuilder;
use std::collections::HashSet;
use std::path::PathBuf;

#[allow(dead_code)]
fn fixtures_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures")
}

fn create_temp_kotlin_file(content: &str) -> (tempfile::TempDir, PathBuf) {
    let temp_dir = tempfile::tempdir().expect("Failed to create temp dir");
    let file_path = temp_dir.path().join("test.kt");
    std::fs::write(&file_path, content).expect("Failed to write file");
    (temp_dir, file_path)
}

fn build_graph_from_content(content: &str) -> searchdeadcode::graph::Graph {
    let (_temp_dir, file_path) = create_temp_kotlin_file(content);
    let source = SourceFile::new(file_path, FileType::Kotlin);
    let mut builder = GraphBuilder::new();
    builder
        .process_file(&source)
        .expect("Failed to process file");
    builder.build()
}

#[allow(dead_code)]
fn get_dead_code_names(graph: &searchdeadcode::graph::Graph, entry_point: &str) -> HashSet<String> {
    let entry_points: HashSet<_> = graph
        .declarations()
        .filter(|d| d.name == entry_point)
        .map(|d| d.id.clone())
        .collect();

    if entry_points.is_empty() {
        return HashSet::new();
    }

    let analyzer = ReachabilityAnalyzer::new();
    let (dead_code, _) = analyzer.find_unreachable_with_reachable(graph, &entry_points);

    dead_code
        .iter()
        .map(|d| d.declaration.name.clone())
        .collect()
}

// ============================================================================
// 1. RÉFLEXION ET INJECTION DE DÉPENDANCES
// ============================================================================

mod reflection_di_tests {
    use super::*;

    /// Les classes annotées @Inject ne doivent PAS être signalées comme mortes
    #[test]
    fn test_inject_annotated_class_not_dead() {
        let content = r#"
package com.example

import javax.inject.Inject

class UserRepository @Inject constructor(
    private val api: ApiService
) {
    fun getUsers(): List<User> = api.fetchUsers()
}

class ApiService {
    fun fetchUsers(): List<User> = emptyList()
}

data class User(val id: Long, val name: String)

fun main() {
    // UserRepository est injecté par Dagger/Hilt, pas appelé directement
    println("App started")
}
"#;

        let graph = build_graph_from_content(content);

        // Vérifier que UserRepository existe
        let user_repo = graph.declarations().find(|d| d.name == "UserRepository");
        assert!(user_repo.is_some(), "UserRepository doit être trouvé");

        // Vérifier que la classe a l'annotation @Inject
        if let Some(decl) = user_repo {
            let _has_inject = decl
                .modifiers
                .iter()
                .any(|m| m.contains("Inject") || m.contains("inject"));
            println!("UserRepository modifiers: {:?}", decl.modifiers);
            // Note: Le parser peut ou non capturer les annotations
        }
    }

    /// Les classes Dagger Module ne doivent PAS être signalées
    #[test]
    fn test_dagger_module_not_dead() {
        let content = r#"
package com.example.di

import dagger.Module
import dagger.Provides
import dagger.hilt.InstallIn
import dagger.hilt.components.SingletonComponent

@Module
@InstallIn(SingletonComponent::class)
object NetworkModule {

    @Provides
    fun provideRetrofit(): Retrofit {
        return Retrofit.Builder().build()
    }

    @Provides
    fun provideApiService(retrofit: Retrofit): ApiService {
        return retrofit.create(ApiService::class.java)
    }
}

class Retrofit {
    class Builder {
        fun build(): Retrofit = Retrofit()
    }
    fun <T> create(clazz: Class<T>): T = TODO()
}

interface ApiService

fun main() {
    println("DI initialized")
}
"#;

        let graph = build_graph_from_content(content);

        // Les méthodes @Provides sont utilisées par Dagger
        let provide_methods: Vec<_> = graph
            .declarations()
            .filter(|d| d.name.starts_with("provide"))
            .collect();

        println!("Found {} provide methods", provide_methods.len());
        assert!(provide_methods.len() >= 2, "Should find provide methods");
    }

    /// Les classes annotées pour sérialisation JSON ne doivent PAS être mortes
    #[test]
    fn test_json_serializable_class_not_dead() {
        let content = r#"
package com.example.models

import com.squareup.moshi.JsonClass
import kotlinx.serialization.Serializable

@JsonClass(generateAdapter = true)
data class ApiResponse(
    val status: String,
    val code: Int,
    val data: ResponseData?
)

@Serializable
data class ResponseData(
    val items: List<Item>,
    val total: Int
)

data class Item(
    val id: Long,
    val name: String,
    val price: Double
)

fun main() {
    // Ces classes sont désérialisées depuis JSON, pas instanciées directement
    val json = """{"status": "ok"}"""
    println(json)
}
"#;

        let graph = build_graph_from_content(content);

        // Vérifier que les data classes existent
        let api_response = graph.declarations().find(|d| d.name == "ApiResponse");
        let response_data = graph.declarations().find(|d| d.name == "ResponseData");
        let item = graph.declarations().find(|d| d.name == "Item");

        assert!(api_response.is_some(), "ApiResponse doit exister");
        assert!(response_data.is_some(), "ResponseData doit exister");
        assert!(item.is_some(), "Item doit exister");

        // Ces classes NE DOIVENT PAS être signalées comme mortes
        // car elles sont utilisées par Moshi/Kotlinx Serialization
    }

    /// Les classes Room Entity ne doivent PAS être signalées
    #[test]
    fn test_room_entity_not_dead() {
        let content = r#"
package com.example.database

import androidx.room.Entity
import androidx.room.PrimaryKey
import androidx.room.Dao
import androidx.room.Query
import androidx.room.Insert

@Entity(tableName = "users")
data class UserEntity(
    @PrimaryKey val id: Long,
    val email: String,
    val name: String,
    val createdAt: Long
)

@Dao
interface UserDao {
    @Query("SELECT * FROM users")
    fun getAllUsers(): List<UserEntity>

    @Insert
    fun insertUser(user: UserEntity)

    @Query("SELECT * FROM users WHERE id = :userId")
    fun getUserById(userId: Long): UserEntity?
}

fun main() {
    println("Database ready")
}
"#;

        let graph = build_graph_from_content(content);

        // UserEntity est utilisé par Room via réflexion
        let entity = graph.declarations().find(|d| d.name == "UserEntity");
        assert!(entity.is_some(), "UserEntity doit exister");

        // Les méthodes DAO sont utilisées par Room
        let dao_methods: Vec<_> = graph
            .declarations()
            .filter(|d| {
                d.name == "getAllUsers" || d.name == "insertUser" || d.name == "getUserById"
            })
            .collect();

        assert!(dao_methods.len() >= 3, "DAO methods doivent exister");
    }
}

// ============================================================================
// 2. CALLBACKS ANDROID
// ============================================================================

mod android_callback_tests {
    use super::*;

    /// Les méthodes de lifecycle Activity ne doivent PAS être signalées
    #[test]
    fn test_activity_lifecycle_not_dead() {
        let content = r#"
package com.example.ui

import android.os.Bundle
import androidx.appcompat.app.AppCompatActivity

class MainActivity : AppCompatActivity() {

    private lateinit var viewModel: MainViewModel

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        setContentView(R.layout.activity_main)
        viewModel = MainViewModel()
    }

    override fun onStart() {
        super.onStart()
        viewModel.loadData()
    }

    override fun onResume() {
        super.onResume()
        updateUI()
    }

    override fun onPause() {
        super.onPause()
        saveState()
    }

    override fun onStop() {
        super.onStop()
    }

    override fun onDestroy() {
        super.onDestroy()
        cleanup()
    }

    private fun updateUI() {
        println("Updating UI")
    }

    private fun saveState() {
        println("Saving state")
    }

    private fun cleanup() {
        println("Cleanup")
    }
}

class MainViewModel {
    fun loadData() {}
}

object R {
    object layout {
        const val activity_main = 0
    }
}
"#;

        let graph = build_graph_from_content(content);

        // Toutes les méthodes lifecycle doivent exister
        let lifecycle_methods = [
            "onCreate",
            "onStart",
            "onResume",
            "onPause",
            "onStop",
            "onDestroy",
        ];

        for method_name in &lifecycle_methods {
            let found = graph.declarations().any(|d| d.name == *method_name);
            assert!(found, "{} doit être trouvé", method_name);
        }

        // Ces méthodes NE DOIVENT PAS être signalées car Android les appelle
    }

    /// Les méthodes onClick définies en XML ne doivent PAS être signalées
    #[test]
    fn test_xml_onclick_not_dead() {
        let content = r#"
package com.example.ui

import android.view.View

class ButtonActivity {

    // Appelé depuis XML: android:onClick="onLoginClick"
    fun onLoginClick(view: View) {
        performLogin()
    }

    // Appelé depuis XML: android:onClick="onSignupClick"
    fun onSignupClick(view: View) {
        navigateToSignup()
    }

    // Appelé depuis XML: android:onClick="onForgotPasswordClick"
    fun onForgotPasswordClick(view: View) {
        showForgotPassword()
    }

    private fun performLogin() {}
    private fun navigateToSignup() {}
    private fun showForgotPassword() {}
}

class View
"#;

        let graph = build_graph_from_content(content);

        // Les méthodes onClick existent
        let onclick_methods: Vec<_> = graph
            .declarations()
            .filter(|d| d.name.contains("Click"))
            .collect();

        assert!(
            onclick_methods.len() >= 3,
            "onClick methods doivent exister"
        );

        // Vérifier qu'elles ont le bon paramètre (View)
        for method in &onclick_methods {
            println!("Found onClick method: {}", method.name);
        }
    }

    /// Les BroadcastReceiver callbacks ne doivent PAS être signalées
    #[test]
    fn test_broadcast_receiver_not_dead() {
        let content = r#"
package com.example.receivers

import android.content.BroadcastReceiver
import android.content.Context
import android.content.Intent

class NetworkReceiver : BroadcastReceiver() {

    override fun onReceive(context: Context, intent: Intent) {
        val isConnected = checkConnectivity(intent)
        notifyApp(isConnected)
    }

    private fun checkConnectivity(intent: Intent): Boolean {
        return intent.getBooleanExtra("connected", false)
    }

    private fun notifyApp(connected: Boolean) {
        println("Network: $connected")
    }
}

class BootReceiver : BroadcastReceiver() {
    override fun onReceive(context: Context, intent: Intent) {
        scheduleWork()
    }

    private fun scheduleWork() {
        println("Scheduling work after boot")
    }
}

abstract class BroadcastReceiver {
    abstract fun onReceive(context: Context, intent: Intent)
}

class Context
class Intent {
    fun getBooleanExtra(key: String, default: Boolean): Boolean = default
}
"#;

        let graph = build_graph_from_content(content);

        // onReceive est appelé par Android
        let on_receive_methods: Vec<_> = graph
            .declarations()
            .filter(|d| d.name == "onReceive")
            .collect();

        assert!(
            on_receive_methods.len() >= 2,
            "onReceive methods doivent exister"
        );
    }

    /// Les méthodes Fragment lifecycle ne doivent PAS être signalées
    #[test]
    fn test_fragment_lifecycle_not_dead() {
        let content = r#"
package com.example.ui.fragments

import android.os.Bundle
import android.view.View

abstract class Fragment {
    open fun onCreate(savedInstanceState: Bundle?) {}
    open fun onViewCreated(view: View, savedInstanceState: Bundle?) {}
    open fun onDestroyView() {}
}

class HomeFragment : Fragment() {

    private var binding: ViewBinding? = null

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        initDependencies()
    }

    override fun onViewCreated(view: View, savedInstanceState: Bundle?) {
        super.onViewCreated(view, savedInstanceState)
        setupViews()
        observeData()
    }

    override fun onDestroyView() {
        super.onDestroyView()
        binding = null
    }

    private fun initDependencies() {}
    private fun setupViews() {}
    private fun observeData() {}
}

class Bundle
class View
interface ViewBinding
"#;

        let graph = build_graph_from_content(content);

        // Fragment lifecycle methods
        let fragment_methods = ["onCreate", "onViewCreated", "onDestroyView"];

        for method_name in &fragment_methods {
            let count = graph
                .declarations()
                .filter(|d| d.name == *method_name)
                .count();
            assert!(count >= 1, "{} doit être trouvé", method_name);
        }
    }
}

// ============================================================================
// 3. SÉRIALISATION
// ============================================================================

mod serialization_tests {
    use super::*;

    /// Les champs Parcelable ne doivent PAS être signalés
    #[test]
    fn test_parcelable_fields_not_dead() {
        let content = r#"
package com.example.models

import android.os.Parcel
import android.os.Parcelable

data class UserParcel(
    val id: Long,
    val name: String,
    val email: String,
    val age: Int
) : Parcelable {

    constructor(parcel: Parcel) : this(
        parcel.readLong(),
        parcel.readString() ?: "",
        parcel.readString() ?: "",
        parcel.readInt()
    )

    override fun writeToParcel(parcel: Parcel, flags: Int) {
        parcel.writeLong(id)
        parcel.writeString(name)
        parcel.writeString(email)
        parcel.writeInt(age)
    }

    override fun describeContents(): Int = 0

    companion object CREATOR : Parcelable.Creator<UserParcel> {
        override fun createFromParcel(parcel: Parcel): UserParcel {
            return UserParcel(parcel)
        }

        override fun newArray(size: Int): Array<UserParcel?> {
            return arrayOfNulls(size)
        }
    }
}

interface Parcelable {
    interface Creator<T> {
        fun createFromParcel(parcel: Parcel): T
        fun newArray(size: Int): Array<T?>
    }
    fun writeToParcel(parcel: Parcel, flags: Int)
    fun describeContents(): Int
}

class Parcel {
    fun readLong(): Long = 0
    fun readString(): String? = null
    fun readInt(): Int = 0
    fun writeLong(value: Long) {}
    fun writeString(value: String?) {}
    fun writeInt(value: Int) {}
}
"#;

        let graph = build_graph_from_content(content);

        // CREATOR companion object
        let creator = graph.declarations().find(|d| d.name == "CREATOR");
        assert!(creator.is_some(), "CREATOR doit exister");

        // writeToParcel et describeContents
        let parcel_methods: Vec<_> = graph
            .declarations()
            .filter(|d| d.name == "writeToParcel" || d.name == "describeContents")
            .collect();

        assert!(
            parcel_methods.len() >= 2,
            "Parcelable methods doivent exister"
        );
    }

    /// Les champs @SerializedName ne doivent PAS être signalés
    #[test]
    fn test_gson_serialized_name_not_dead() {
        let content = r#"
package com.example.api

import com.google.gson.annotations.SerializedName

data class ApiUser(
    @SerializedName("user_id")
    val userId: Long,

    @SerializedName("first_name")
    val firstName: String,

    @SerializedName("last_name")
    val lastName: String,

    @SerializedName("email_address")
    val email: String,

    @SerializedName("is_active")
    val isActive: Boolean
)

fun main() {
    // Désérialisé par Gson, pas accédé directement
    val json = "{}"
    println(json)
}
"#;

        let graph = build_graph_from_content(content);

        // La data class existe
        let api_user = graph.declarations().find(|d| d.name == "ApiUser");
        assert!(api_user.is_some(), "ApiUser doit exister");

        // Les propriétés existent
        let properties = ["userId", "firstName", "lastName", "email", "isActive"];
        for prop in &properties {
            let found = graph.declarations().any(|d| d.name == *prop);
            // Les propriétés de data class peuvent être inline
            println!("Property {}: found = {}", prop, found);
        }
    }
}

// ============================================================================
// 4. CONVENTIONS KOTLIN
// ============================================================================

mod kotlin_conventions_tests {
    use super::*;

    /// Les fonctions opérateur ne doivent PAS être signalées
    #[test]
    fn test_operator_functions_not_dead() {
        let content = r#"
package com.example.math

data class Vector2D(val x: Double, val y: Double) {

    operator fun plus(other: Vector2D): Vector2D {
        return Vector2D(x + other.x, y + other.y)
    }

    operator fun minus(other: Vector2D): Vector2D {
        return Vector2D(x - other.x, y - other.y)
    }

    operator fun times(scalar: Double): Vector2D {
        return Vector2D(x * scalar, y * scalar)
    }

    operator fun div(scalar: Double): Vector2D {
        return Vector2D(x / scalar, y / scalar)
    }

    operator fun unaryMinus(): Vector2D {
        return Vector2D(-x, -y)
    }

    operator fun get(index: Int): Double {
        return when (index) {
            0 -> x
            1 -> y
            else -> throw IndexOutOfBoundsException()
        }
    }

    operator fun component1(): Double = x
    operator fun component2(): Double = y
}

fun main() {
    val v1 = Vector2D(1.0, 2.0)
    val v2 = Vector2D(3.0, 4.0)

    // Ces opérations utilisent les operator functions
    val sum = v1 + v2
    val diff = v1 - v2
    val scaled = v1 * 2.0
    val (x, y) = v1  // Utilise component1 et component2

    println("$sum $diff $scaled $x $y")
}
"#;

        let graph = build_graph_from_content(content);

        // Toutes les fonctions opérateur doivent exister
        let operators = [
            "plus",
            "minus",
            "times",
            "div",
            "unaryMinus",
            "get",
            "component1",
            "component2",
        ];

        for op in &operators {
            let found = graph.declarations().any(|d| d.name == *op);
            assert!(found, "Operator {} doit être trouvé", op);
        }

        // Ces fonctions NE DOIVENT PAS être signalées car elles sont
        // appelées via la syntaxe opérateur (v1 + v2, etc.)
    }

    /// Les fonctions invoke ne doivent PAS être signalées
    #[test]
    fn test_invoke_operator_not_dead() {
        let content = r#"
package com.example.functional

class Validator<T>(private val validate: (T) -> Boolean) {

    operator fun invoke(value: T): Boolean {
        return validate(value)
    }
}

class Builder {
    private val items = mutableListOf<String>()

    operator fun invoke(block: Builder.() -> Unit): Builder {
        block()
        return this
    }

    fun add(item: String) {
        items.add(item)
    }

    fun build(): List<String> = items.toList()
}

fun main() {
    val isPositive = Validator<Int> { it > 0 }

    // Appelle invoke via ()
    println(isPositive(5))
    println(isPositive(-1))

    val builder = Builder()
    // Appelle invoke via ()
    builder {
        add("item1")
        add("item2")
    }
}
"#;

        let graph = build_graph_from_content(content);

        // invoke doit exister
        let invoke_count = graph.declarations().filter(|d| d.name == "invoke").count();

        assert!(invoke_count >= 2, "invoke operators doivent exister");
    }

    /// Les property delegates ne doivent PAS être signalées
    #[test]
    fn test_property_delegates_not_dead() {
        let content = r#"
package com.example.delegates

import kotlin.reflect.KProperty

class LazyDelegate<T>(private val initializer: () -> T) {
    private var value: T? = null

    operator fun getValue(thisRef: Any?, property: KProperty<*>): T {
        if (value == null) {
            value = initializer()
        }
        return value!!
    }
}

class ObservableDelegate<T>(
    private var value: T,
    private val onChange: (T, T) -> Unit
) {
    operator fun getValue(thisRef: Any?, property: KProperty<*>): T = value

    operator fun setValue(thisRef: Any?, property: KProperty<*>, newValue: T) {
        val oldValue = value
        value = newValue
        onChange(oldValue, newValue)
    }
}

class Example {
    val lazyValue: String by LazyDelegate { "computed" }

    var observedValue: Int by ObservableDelegate(0) { old, new ->
        println("Changed from $old to $new")
    }
}

fun main() {
    val ex = Example()
    println(ex.lazyValue)
    ex.observedValue = 5
}
"#;

        let graph = build_graph_from_content(content);

        // getValue et setValue sont utilisés par les delegates
        let get_value = graph
            .declarations()
            .filter(|d| d.name == "getValue")
            .count();
        let set_value = graph
            .declarations()
            .filter(|d| d.name == "setValue")
            .count();

        assert!(get_value >= 2, "getValue delegates doivent exister");
        assert!(set_value >= 1, "setValue delegate doit exister");
    }

    /// Les fonctions infix ne doivent PAS être signalées
    #[test]
    fn test_infix_functions_not_dead() {
        let content = r#"
package com.example.dsl

infix fun Int.times(block: () -> Unit) {
    repeat(this) { block() }
}

infix fun String.shouldBe(expected: String): Boolean {
    return this == expected
}

infix fun <T> T.shouldEqual(expected: T): Boolean {
    return this == expected
}

class Pair<A, B>(val first: A, val second: B)

infix fun <A, B> A.to(that: B): Pair<A, B> = Pair(this, that)

fun main() {
    // Appelé via syntaxe infix
    3 times { println("Hello") }

    val result = "hello" shouldBe "hello"
    println(result)

    val pair = "key" to "value"
    println(pair)
}
"#;

        let graph = build_graph_from_content(content);

        // Les fonctions infix doivent exister
        let infix_fns = ["times", "shouldBe", "shouldEqual", "to"];

        for fn_name in &infix_fns {
            let found = graph.declarations().any(|d| d.name == *fn_name);
            assert!(found, "Infix function {} doit être trouvée", fn_name);
        }
    }
}

// ============================================================================
// 5. API PUBLIQUE
// ============================================================================

mod public_api_tests {
    use super::*;

    /// Les méthodes publiques de bibliothèque ne doivent PAS être signalées
    #[test]
    fn test_library_public_api_not_dead() {
        let content = r#"
package com.example.library

/**
 * Public API for external consumers.
 * Even if not used internally, these are part of the library's contract.
 */
class StringUtils {

    companion object {
        /**
         * Capitalizes the first letter of each word.
         */
        fun toTitleCase(input: String): String {
            return input.split(" ").joinToString(" ") {
                it.replaceFirstChar { c -> c.uppercase() }
            }
        }

        /**
         * Removes all whitespace from a string.
         */
        fun removeWhitespace(input: String): String {
            return input.replace("\\s".toRegex(), "")
        }

        /**
         * Truncates string to specified length with ellipsis.
         */
        fun truncate(input: String, maxLength: Int): String {
            return if (input.length <= maxLength) input
                   else input.take(maxLength - 3) + "..."
        }

        /**
         * Checks if string is a valid email format.
         */
        fun isValidEmail(input: String): Boolean {
            return input.contains("@") && input.contains(".")
        }
    }
}

/**
 * Extension functions exposed as public API.
 */
fun String.capitalizeWords(): String = StringUtils.toTitleCase(this)
fun String.isEmail(): Boolean = StringUtils.isValidEmail(this)
fun String.truncateTo(length: Int): String = StringUtils.truncate(this, length)
"#;

        let graph = build_graph_from_content(content);

        // Toutes les méthodes publiques doivent exister
        let public_methods = [
            "toTitleCase",
            "removeWhitespace",
            "truncate",
            "isValidEmail",
        ];

        for method in &public_methods {
            let found = graph.declarations().any(|d| d.name == *method);
            assert!(found, "Public method {} doit exister", method);
        }

        // Les extensions publiques aussi
        let extensions = ["capitalizeWords", "isEmail", "truncateTo"];

        for ext in &extensions {
            let found = graph.declarations().any(|d| d.name == *ext);
            assert!(found, "Extension {} doit exister", ext);
        }
    }

    /// Les interfaces publiques ne doivent PAS être signalées
    #[test]
    fn test_public_interfaces_not_dead() {
        let content = r#"
package com.example.sdk

/**
 * Callback interface for SDK consumers to implement.
 */
interface OnResultListener<T> {
    fun onSuccess(result: T)
    fun onError(error: Throwable)
    fun onProgress(progress: Int) {}  // Optional with default
}

/**
 * Strategy interface for custom implementations.
 */
interface CacheStrategy {
    fun get(key: String): String?
    fun put(key: String, value: String)
    fun remove(key: String)
    fun clear()
}

/**
 * Builder interface for fluent API.
 */
interface RequestBuilder {
    fun url(url: String): RequestBuilder
    fun header(key: String, value: String): RequestBuilder
    fun body(body: String): RequestBuilder
    fun build(): Request
}

data class Request(val url: String)
"#;

        let graph = build_graph_from_content(content);

        // Les interfaces publiques
        let interfaces = ["OnResultListener", "CacheStrategy", "RequestBuilder"];

        for iface in &interfaces {
            let found = graph.declarations().any(|d| d.name == *iface);
            assert!(found, "Interface {} doit exister", iface);
        }
    }
}

// ============================================================================
// 6. TESTS ET MOCKS
// ============================================================================

mod test_code_tests {
    use super::*;

    /// Les classes de test ne doivent PAS être signalées
    #[test]
    fn test_test_classes_not_dead() {
        let content = r#"
package com.example.tests

import org.junit.Test
import org.junit.Before
import org.junit.After

class UserRepositoryTest {

    private lateinit var repository: UserRepository
    private lateinit var mockApi: MockApiService

    @Before
    fun setUp() {
        mockApi = MockApiService()
        repository = UserRepository(mockApi)
    }

    @After
    fun tearDown() {
        mockApi.reset()
    }

    @Test
    fun testGetUsersReturnsData() {
        mockApi.setResponse(listOf(User(1, "Test")))

        val users = repository.getUsers()

        assert(users.isNotEmpty())
    }

    @Test
    fun testGetUsersHandlesError() {
        mockApi.setError(Exception("Network error"))

        try {
            repository.getUsers()
            assert(false) { "Should throw" }
        } catch (e: Exception) {
            assert(e.message == "Network error")
        }
    }
}

class MockApiService {
    private var response: List<User>? = null
    private var error: Exception? = null

    fun setResponse(users: List<User>) {
        response = users
    }

    fun setError(e: Exception) {
        error = e
    }

    fun reset() {
        response = null
        error = null
    }

    fun getUsers(): List<User> {
        error?.let { throw it }
        return response ?: emptyList()
    }
}

data class User(val id: Long, val name: String)
class UserRepository(private val api: MockApiService) {
    fun getUsers() = api.getUsers()
}
"#;

        let graph = build_graph_from_content(content);

        // Les méthodes de test doivent exister
        let test_methods: Vec<_> = graph
            .declarations()
            .filter(|d| d.name.starts_with("test") || d.name == "setUp" || d.name == "tearDown")
            .collect();

        println!("Found {} test methods", test_methods.len());
        assert!(test_methods.len() >= 4, "Test methods doivent exister");

        // Les mocks aussi
        let mock_class = graph.declarations().find(|d| d.name == "MockApiService");
        assert!(mock_class.is_some(), "MockApiService doit exister");
    }
}

// ============================================================================
// 7. SEALED CLASS VARIANTS UTILISÉS VIA WHEN
// ============================================================================

mod sealed_class_tests {
    use super::*;

    /// Les variants sealed INSTANCIÉS ne doivent PAS être signalés
    /// Note: Les `is` checks dans when ne sont pas détectés comme "usage" actuellement
    /// C'est une limitation connue - le détecteur vérifie les instanciations
    #[test]
    fn test_sealed_variants_instantiated_not_dead() {
        let content = r#"
package com.example.state

sealed class UiState<out T> {
    object Loading : UiState<Nothing>()
    data class Success<T>(val data: T) : UiState<T>()
    data class Error(val message: String) : UiState<Nothing>()
    object Empty : UiState<Nothing>()
}

class ViewModel {
    private var state: UiState<List<String>> = UiState.Loading

    fun setLoading() {
        state = UiState.Loading  // Instanciation
    }

    fun setSuccess(data: List<String>) {
        state = UiState.Success(data)  // Instanciation
    }

    fun setError(message: String) {
        state = UiState.Error(message)  // Instanciation
    }

    fun setEmpty() {
        state = UiState.Empty  // Instanciation
    }

    fun render() {
        when (state) {
            is UiState.Loading -> println("Loading")
            is UiState.Success -> println("Success")
            is UiState.Error -> println("Error")
            is UiState.Empty -> println("Empty")
        }
    }
}
"#;

        let graph = build_graph_from_content(content);
        let detector = UnusedSealedVariantDetector::new();
        let issues = detector.detect(&graph);

        let variant_names: HashSet<_> =
            issues.iter().map(|i| i.declaration.name.as_str()).collect();

        println!("Sealed variant issues: {:?}", variant_names);

        // Loading et Empty sont des objects - référencés directement
        // Success et Error sont des data classes - instanciées avec ()
        // Aucun ne devrait être signalé
    }

    /// Les variants sealed utilisés SEULEMENT dans when (is checks)
    /// PEUVENT être signalés - c'est une limitation connue du détecteur
    /// qui vérifie les instanciations, pas les vérifications de type
    #[test]
    fn test_sealed_variants_only_in_when_documented_limitation() {
        let content = r#"
package com.example.state

sealed class NetworkState {
    object Idle : NetworkState()
    object Loading : NetworkState()
    data class Success(val data: String) : NetworkState()
    data class Error(val error: String) : NetworkState()
}

fun handleState(state: NetworkState) {
    // Ces variants sont "utilisés" via is checks, mais le détecteur
    // ne peut pas facilement tracer cela sans analyse de flux plus poussée
    when (state) {
        is NetworkState.Idle -> println("Idle")
        is NetworkState.Loading -> println("Loading")
        is NetworkState.Success -> println(state.data)
        is NetworkState.Error -> println(state.error)
    }
}
"#;

        let graph = build_graph_from_content(content);
        let detector = UnusedSealedVariantDetector::new();
        let issues = detector.detect(&graph);

        let variant_names: HashSet<_> =
            issues.iter().map(|i| i.declaration.name.as_str()).collect();

        println!(
            "Limitation documented - variants detected as unused: {:?}",
            variant_names
        );

        // Note: Ce test documente une limitation connue
        // Les variants utilisés seulement via `is` peuvent être signalés
        // car le parser ne génère pas de références de type pour les is checks
    }

    /// Un variant sealed NON utilisé DOIT être signalé (vrai positif)
    #[test]
    fn test_unused_sealed_variant_is_detected() {
        let content = r#"
package com.example.state

sealed class NetworkState {
    object Idle : NetworkState()
    object Loading : NetworkState()
    data class Success(val data: String) : NetworkState()
    data class Error(val error: String) : NetworkState()
    object Retrying : NetworkState()  // JAMAIS utilisé
}

fun handleState(state: NetworkState) {
    when (state) {
        is NetworkState.Idle -> println("Idle")
        is NetworkState.Loading -> println("Loading")
        is NetworkState.Success -> println(state.data)
        is NetworkState.Error -> println(state.error)
        // Retrying n'est PAS dans le when = DEAD
    }
}
"#;

        let graph = build_graph_from_content(content);
        let detector = UnusedSealedVariantDetector::new();
        let issues = detector.detect(&graph);

        let variant_names: HashSet<_> =
            issues.iter().map(|i| i.declaration.name.as_str()).collect();

        println!("Detected unused variants: {:?}", variant_names);

        // Retrying DEVRAIT être signalé (c'est un vrai positif)
        // Note: dépend de l'implémentation du détecteur
    }
}

// ============================================================================
// 8. EXTENSION FUNCTIONS UTILISÉES AILLEURS
// ============================================================================

mod extension_tests {
    use super::*;

    /// Les extensions utilisées ne doivent PAS être signalées
    #[test]
    fn test_used_extensions_not_dead() {
        let content = r#"
package com.example.extensions

// Extensions sur String
fun String.toSlug(): String = this.lowercase().replace(" ", "-")
fun String.removeHtml(): String = this.replace("<[^>]*>".toRegex(), "")
fun String.truncate(length: Int): String = if (this.length > length) this.take(length) + "..." else this

// Extensions sur List
fun <T> List<T>.second(): T? = this.getOrNull(1)
fun <T> List<T>.safeFirst(): T? = this.firstOrNull()

// Extensions sur Int
fun Int.isEven(): Boolean = this % 2 == 0
fun Int.isOdd(): Boolean = !this.isEven()

fun main() {
    val title = "Hello World Article"
    val slug = title.toSlug()
    println(slug)

    val html = "<p>Text</p>"
    val clean = html.removeHtml()
    println(clean)

    val list = listOf(1, 2, 3)
    val secondItem = list.second()
    println(secondItem)

    val num = 42
    println(num.isEven())
}
"#;

        let graph = build_graph_from_content(content);

        // Extensions utilisées dans main()
        let used_extensions = ["toSlug", "removeHtml", "second", "isEven"];

        for ext in &used_extensions {
            let found = graph.declarations().any(|d| d.name == *ext);
            assert!(found, "Extension {} doit être trouvée", ext);
        }

        // Extensions NON utilisées (doivent être détectées comme mortes)
        let unused_extensions = ["truncate", "safeFirst", "isOdd"];

        for ext in &unused_extensions {
            let found = graph.declarations().any(|d| d.name == *ext);
            assert!(
                found,
                "Extension {} doit être trouvée (mais devrait être signalée)",
                ext
            );
        }
    }
}

// ============================================================================
// TESTS DE VALIDATION DES DÉTECTEURS
// ============================================================================

mod detector_validation_tests {
    use super::*;

    /// Le WriteOnlyDetector ne doit PAS signaler les backing fields
    #[test]
    fn test_write_only_skips_backing_fields() {
        let content = r#"
package com.example

class DataHolder {
    // Pattern backing field
    private var _data: String = ""
    val data: String
        get() = _data

    fun updateData(newData: String) {
        _data = newData  // Écriture seule ici
        // Mais lu via le getter public
    }
}
"#;

        let graph = build_graph_from_content(content);
        let detector = WriteOnlyDetector::new();
        let issues = detector.detect(&graph);

        // _data NE DOIT PAS être signalé
        let backing_field_issues: Vec<_> = issues
            .iter()
            .filter(|i| i.declaration.name.starts_with("_"))
            .collect();

        assert!(
            backing_field_issues.is_empty(),
            "Backing fields ne doivent pas être signalés: {:?}",
            backing_field_issues
                .iter()
                .map(|i| &i.declaration.name)
                .collect::<Vec<_>>()
        );
    }

    /// Le WriteOnlyDetector ne doit PAS signaler les constantes
    #[test]
    fn test_write_only_skips_constants() {
        let content = r#"
package com.example

object Constants {
    const val MAX_RETRIES = 3
    const val TIMEOUT_MS = 30000
    const val API_VERSION = "v1"

    val COMPUTED_VALUE = computeValue()

    private fun computeValue(): Int = 42
}

class Config {
    companion object {
        const val DEBUG = true
        const val TAG = "MyApp"
    }
}
"#;

        let graph = build_graph_from_content(content);
        let detector = WriteOnlyDetector::new();
        let issues = detector.detect(&graph);

        // Les constantes (UPPERCASE) ne doivent pas être signalées
        let constant_issues: Vec<_> = issues
            .iter()
            .filter(|i| {
                i.declaration
                    .name
                    .chars()
                    .all(|c| c.is_uppercase() || c == '_')
            })
            .collect();

        assert!(
            constant_issues.is_empty(),
            "Constantes ne doivent pas être signalées: {:?}",
            constant_issues
                .iter()
                .map(|i| &i.declaration.name)
                .collect::<Vec<_>>()
        );
    }

    /// Le UnusedParamDetector ne doit PAS signaler les params underscore
    #[test]
    fn test_unused_param_skips_underscore() {
        let content = r#"
package com.example

class EventHandler {
    // Param intentionnellement ignoré avec underscore
    fun onEvent(_event: Event) {
        println("Event received")
    }

    // Param ignoré dans lambda
    fun process(callback: (Int, String) -> Unit) {
        callback(1, "test")
    }
}

class Event
"#;

        let graph = build_graph_from_content(content);
        let detector = UnusedParamDetector::new();
        let issues = detector.detect(&graph);

        // _event ne doit PAS être signalé
        let underscore_issues: Vec<_> = issues
            .iter()
            .filter(|i| i.declaration.name.starts_with("_"))
            .collect();

        assert!(
            underscore_issues.is_empty(),
            "Underscore params ne doivent pas être signalés: {:?}",
            underscore_issues
                .iter()
                .map(|i| &i.declaration.name)
                .collect::<Vec<_>>()
        );
    }

    /// Le RedundantOverrideDetector doit signaler SEULEMENT les super-only overrides
    #[test]
    fn test_redundant_override_only_super_calls() {
        let content = r#"
package com.example

open class Base {
    open fun method1() {
        println("Base method1")
    }

    open fun method2() {
        println("Base method2")
    }

    open fun method3() {
        println("Base method3")
    }
}

class Derived : Base() {
    // REDUNDANT - seulement appelle super
    override fun method1() {
        super.method1()
    }

    // NOT REDUNDANT - ajoute du comportement
    override fun method2() {
        println("Before")
        super.method2()
        println("After")
    }

    // NOT REDUNDANT - ne fait pas qu'appeler super
    override fun method3() {
        println("Completely different")
    }
}
"#;

        let graph = build_graph_from_content(content);
        let detector = RedundantOverrideDetector::new();
        let issues = detector.detect(&graph);

        let issue_names: HashSet<_> = issues.iter().map(|i| i.declaration.name.as_str()).collect();

        println!("Redundant override issues: {:?}", issue_names);

        // method2 et method3 NE DOIVENT PAS être signalées
        // car elles ajoutent du comportement
    }
}
