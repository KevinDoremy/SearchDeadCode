package com.example.antipattern

// BAD: Method with too many lines (>50)
class LongMethodExample {

    fun veryLongMethod(data: List<Item>): Result {
        // Line 1
        val result = mutableListOf<ProcessedItem>()
        // Line 2
        var totalCount = 0
        // Line 3
        var errorCount = 0
        // Line 4
        var successCount = 0
        // Line 5
        val startTime = System.currentTimeMillis()
        // Line 6-10
        println("Starting processing")
        println("Data size: ${data.size}")
        println("Time: $startTime")
        println("Processing...")
        println("Please wait...")
        // Line 11-20
        for (item in data) {
            try {
                val processed = processItem(item)
                result.add(processed)
                successCount++
                totalCount++
                println("Processed: ${item.id}")
            } catch (e: Exception) {
                errorCount++
                totalCount++
                println("Error: ${e.message}")
            }
        }
        // Line 21-30
        val endTime = System.currentTimeMillis()
        val duration = endTime - startTime
        println("Processing complete")
        println("Duration: $duration ms")
        println("Total: $totalCount")
        println("Success: $successCount")
        println("Errors: $errorCount")
        val successRate = if (totalCount > 0) successCount.toDouble() / totalCount else 0.0
        println("Success rate: $successRate")
        // Line 31-40
        if (errorCount > 0) {
            println("Some errors occurred")
            logErrors(errorCount)
            notifyAdmin(errorCount)
            scheduleRetry()
        }
        if (successRate < 0.5) {
            println("Low success rate!")
            alertOps()
        }
        // Line 41-50
        val summary = Summary(
            total = totalCount,
            success = successCount,
            errors = errorCount,
            duration = duration,
            rate = successRate
        )
        saveSummary(summary)
        notifyComplete(summary)
        cleanup()
        // Line 51+ (exceeds threshold)
        return Result(
            items = result,
            summary = summary,
            timestamp = endTime
        )
    }

    // GOOD: Short, focused method
    fun shortMethod(item: Item): ProcessedItem {
        validate(item)
        val transformed = transform(item)
        return ProcessedItem(transformed)
    }

    // GOOD: Method broken into smaller pieces
    fun wellStructuredMethod(data: List<Item>): Result {
        val processed = processAll(data)
        val summary = calculateSummary(processed)
        return createResult(processed, summary)
    }

    private fun processItem(item: Item): ProcessedItem = ProcessedItem(item.id)
    private fun logErrors(count: Int) {}
    private fun notifyAdmin(count: Int) {}
    private fun scheduleRetry() {}
    private fun alertOps() {}
    private fun saveSummary(summary: Summary) {}
    private fun notifyComplete(summary: Summary) {}
    private fun cleanup() {}
    private fun validate(item: Item) {}
    private fun transform(item: Item): String = item.id
    private fun processAll(data: List<Item>): List<ProcessedItem> = emptyList()
    private fun calculateSummary(items: List<ProcessedItem>): Summary = Summary(0, 0, 0, 0, 0.0)
    private fun createResult(items: List<ProcessedItem>, summary: Summary): Result = Result(items, summary, 0)
}

data class Item(val id: String)
data class ProcessedItem(val id: String)
data class Summary(val total: Int, val success: Int, val errors: Int, val duration: Long, val rate: Double)
data class Result(val items: List<ProcessedItem>, val summary: Summary, val timestamp: Long)
