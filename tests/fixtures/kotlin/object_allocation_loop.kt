// Test fixture for ObjectAllocationInLoopDetector (AP015)
// Detects object allocation inside loops (especially in onDraw)

package com.example.antipattern

import android.graphics.*
import android.view.View
import android.content.Context
import android.util.AttributeSet

// BAD: Object allocation in onDraw (called 60x per second!)
class BadCustomView @JvmOverloads constructor(
    context: Context,
    attrs: AttributeSet? = null
) : View(context, attrs) {

    override fun onDraw(canvas: Canvas) {
        super.onDraw(canvas)

        // BAD: Creating Paint every frame
        val paint = Paint()
        paint.color = Color.RED
        paint.style = Paint.Style.FILL

        // BAD: Creating Rect every frame
        val rect = Rect(0, 0, width, height)
        canvas.drawRect(rect, paint)

        // BAD: Creating Path every frame
        val path = Path()
        path.moveTo(0f, 0f)
        path.lineTo(width.toFloat(), height.toFloat())
        canvas.drawPath(path, paint)

        // BAD: Creating multiple objects in loop inside onDraw
        for (i in 0 until 10) {
            val innerPaint = Paint()
            innerPaint.color = Color.BLUE
            canvas.drawCircle(i * 10f, i * 10f, 5f, innerPaint)
        }
    }
}

// BAD: Object allocation in loops
class LoopAllocator {

    // BAD: Creating objects inside for loop
    fun processInLoop(items: List<Item>) {
        for (item in items) {
            val builder = StringBuilder()  // Allocated every iteration
            builder.append(item.name)
            println(builder.toString())
        }
    }

    // BAD: Creating objects inside while loop
    fun whileLoopAllocation(count: Int) {
        var i = 0
        while (i < count) {
            val data = DataObject(i)  // Allocated every iteration
            process(data)
            i++
        }
    }

    // BAD: Creating objects inside forEach
    fun forEachAllocation(items: List<Item>) {
        items.forEach { item ->
            val wrapper = ItemWrapper(item)  // Allocated every iteration
            handleWrapper(wrapper)
        }
    }

    // BAD: Point/Rect allocation in animation loop
    fun animationLoop(view: View) {
        for (frame in 0 until 60) {
            val point = Point(frame, frame)  // Allocated every frame
            val bounds = Rect(0, 0, 100, 100)  // Allocated every frame
            view.layout(bounds.left, bounds.top, bounds.right, bounds.bottom)
        }
    }
}

// GOOD: Pre-allocate objects outside loops/onDraw
class GoodCustomView @JvmOverloads constructor(
    context: Context,
    attrs: AttributeSet? = null
) : View(context, attrs) {

    // Pre-allocated objects
    private val paint = Paint().apply {
        color = Color.RED
        style = Paint.Style.FILL
    }
    private val rect = Rect()
    private val path = Path()
    private val circlePaint = Paint().apply {
        color = Color.BLUE
    }

    override fun onDraw(canvas: Canvas) {
        super.onDraw(canvas)

        // GOOD: Reuse pre-allocated objects
        rect.set(0, 0, width, height)
        canvas.drawRect(rect, paint)

        // GOOD: Reset and reuse path
        path.reset()
        path.moveTo(0f, 0f)
        path.lineTo(width.toFloat(), height.toFloat())
        canvas.drawPath(path, paint)

        // GOOD: Reuse paint in loop
        for (i in 0 until 10) {
            canvas.drawCircle(i * 10f, i * 10f, 5f, circlePaint)
        }
    }
}

// GOOD: Pre-allocate outside loops
class GoodLoopHandler {

    private val reusableBuilder = StringBuilder()
    private val reusablePoint = Point()
    private val reusableBounds = Rect()

    fun processEfficiently(items: List<Item>) {
        for (item in items) {
            reusableBuilder.clear()
            reusableBuilder.append(item.name)
            println(reusableBuilder.toString())
        }
    }

    fun efficientAnimationLoop(view: View) {
        for (frame in 0 until 60) {
            reusablePoint.set(frame, frame)
            reusableBounds.set(0, 0, 100, 100)
            view.layout(
                reusableBounds.left,
                reusableBounds.top,
                reusableBounds.right,
                reusableBounds.bottom
            )
        }
    }
}

// Supporting classes
data class Item(val name: String)
data class DataObject(val id: Int)
data class ItemWrapper(val item: Item)

fun process(data: DataObject) {}
fun handleWrapper(wrapper: ItemWrapper) {}
