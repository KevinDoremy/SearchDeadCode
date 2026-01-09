// Test fixture for NavControllerPassingDetector (AP034)
// Detects NavController passed to child composables

package com.example.compose

import androidx.compose.runtime.*
import androidx.compose.material.*
import androidx.navigation.*
import androidx.navigation.compose.*

// BAD: Passing NavController to child composable
@Composable
fun BadNavigation() {
    val navController = rememberNavController()

    NavHost(navController = navController, startDestination = "home") {
        composable("home") {
            HomeScreen(navController = navController)  // BAD: Passing NavController
        }
        composable("details/{id}") { backStackEntry ->
            val id = backStackEntry.arguments?.getString("id")
            DetailsScreen(
                id = id,
                navController = navController  // BAD
            )
        }
    }
}

// BAD: NavController passed through multiple layers
@Composable
fun HomeScreen(navController: NavController) {  // BAD: Receives NavController
    Column {
        Text("Home")
        ItemList(navController = navController)  // BAD: Passes it down
    }
}

@Composable
fun ItemList(navController: NavController) {  // BAD: Receives NavController
    LazyColumn {
        items(10) { index ->
            ItemCard(
                index = index,
                navController = navController  // BAD: Passes it further
            )
        }
    }
}

@Composable
fun ItemCard(index: Int, navController: NavController) {  // BAD
    Card(
        onClick = { navController.navigate("details/$index") }
    ) {
        Text("Item $index")
    }
}

// BAD: NavController stored in state
@Composable
fun BadNavControllerState() {
    var navController by remember { mutableStateOf<NavController?>(null) }

    navController?.let { nav ->
        SomeChild(nav)  // BAD
    }
}

// GOOD: Using navigation callbacks
@Composable
fun GoodNavigation() {
    val navController = rememberNavController()

    NavHost(navController = navController, startDestination = "home") {
        composable("home") {
            HomeScreenGood(
                onNavigateToDetails = { id -> navController.navigate("details/$id") }
            )
        }
        composable("details/{id}") { backStackEntry ->
            val id = backStackEntry.arguments?.getString("id")
            DetailsScreenGood(
                id = id,
                onNavigateBack = { navController.popBackStack() }
            )
        }
    }
}

// GOOD: Composable receives navigation callback
@Composable
fun HomeScreenGood(onNavigateToDetails: (String) -> Unit) {
    Column {
        Text("Home")
        ItemListGood(onItemClick = onNavigateToDetails)
    }
}

@Composable
fun ItemListGood(onItemClick: (String) -> Unit) {
    LazyColumn {
        items(10) { index ->
            ItemCardGood(
                index = index,
                onClick = { onItemClick(index.toString()) }
            )
        }
    }
}

@Composable
fun ItemCardGood(index: Int, onClick: () -> Unit) {
    Card(onClick = onClick) {
        Text("Item $index")
    }
}

// GOOD: Using sealed class for navigation events
sealed class NavigationEvent {
    data class ToDetails(val id: String) : NavigationEvent()
    object Back : NavigationEvent()
}

@Composable
fun GoodEventNavigation() {
    val navController = rememberNavController()

    val onNavigate: (NavigationEvent) -> Unit = { event ->
        when (event) {
            is NavigationEvent.ToDetails -> navController.navigate("details/${event.id}")
            NavigationEvent.Back -> navController.popBackStack()
        }
    }

    NavHost(navController = navController, startDestination = "home") {
        composable("home") {
            EventHomeScreen(onNavigate = onNavigate)
        }
    }
}

@Composable
fun EventHomeScreen(onNavigate: (NavigationEvent) -> Unit) {
    Button(onClick = { onNavigate(NavigationEvent.ToDetails("123")) }) {
        Text("Go to details")
    }
}

// GOOD: Navigation handled at NavHost level only
@Composable
fun DetailsScreenGood(
    id: String?,
    onNavigateBack: () -> Unit
) {
    Column {
        IconButton(onClick = onNavigateBack) {
            Icon(Icons.Default.ArrowBack, "Back")
        }
        Text("Details for $id")
    }
}
