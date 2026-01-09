// Test fixture for InitOnDrawDetector (AP030)
// Detects object allocation in onDraw()

package com.example.antipattern

import android.content.Context
import android.graphics.*
import android.util.AttributeSet
import android.view.View

// BAD: Object allocation in onDraw (called 60x per second!)
class BadCustomView @JvmOverloads constructor(
    context: Context,
    attrs: AttributeSet? = null,
    defStyleAttr: Int = 0
) : View(context, attrs, defStyleAttr) {

    override fun onDraw(canvas: Canvas) {
        super.onDraw(canvas)

        // BAD: Creating Paint every frame
        val paint = Paint()
        paint.color = Color.RED
        paint.style = Paint.Style.FILL
        paint.isAntiAlias = true

        // BAD: Creating Rect every frame
        val rect = Rect(0, 0, width, height)

        // BAD: Creating RectF every frame
        val rectF = RectF(10f, 10f, width - 10f, height - 10f)

        // BAD: Creating Path every frame
        val path = Path()
        path.moveTo(0f, 0f)
        path.lineTo(width.toFloat(), height.toFloat())

        // BAD: Creating Matrix every frame
        val matrix = Matrix()
        matrix.setRotate(45f)

        // BAD: Creating LinearGradient every frame
        val gradient = LinearGradient(
            0f, 0f, width.toFloat(), height.toFloat(),
            Color.RED, Color.BLUE, Shader.TileMode.CLAMP
        )

        // BAD: String formatting in onDraw
        val text = String.format("Size: %dx%d", width, height)

        // BAD: Creating arrays in onDraw
        val points = floatArrayOf(0f, 0f, width.toFloat(), height.toFloat())

        canvas.drawRect(rect, paint)
        canvas.drawPath(path, paint)
        canvas.drawText(text, 100f, 100f, paint)
    }
}

// BAD: Allocations in dispatchDraw
class BadViewGroup(context: Context) : ViewGroup(context, null) {

    override fun onLayout(changed: Boolean, l: Int, t: Int, r: Int, b: Int) {}

    override fun dispatchDraw(canvas: Canvas) {
        // BAD: Allocation in dispatchDraw
        val clipRect = Rect()
        canvas.getClipBounds(clipRect)

        super.dispatchDraw(canvas)
    }
}

// GOOD: Pre-allocate objects as instance fields
class GoodCustomView @JvmOverloads constructor(
    context: Context,
    attrs: AttributeSet? = null,
    defStyleAttr: Int = 0
) : View(context, attrs, defStyleAttr) {

    // Pre-allocated paint objects
    private val fillPaint = Paint().apply {
        color = Color.RED
        style = Paint.Style.FILL
        isAntiAlias = true
    }

    private val strokePaint = Paint().apply {
        color = Color.BLUE
        style = Paint.Style.STROKE
        strokeWidth = 2f
    }

    // Pre-allocated geometry objects
    private val rect = Rect()
    private val rectF = RectF()
    private val path = Path()
    private val matrix = Matrix()

    // Pre-allocated for text
    private val textBuilder = StringBuilder()

    override fun onDraw(canvas: Canvas) {
        super.onDraw(canvas)

        // GOOD: Reuse pre-allocated Rect
        rect.set(0, 0, width, height)

        // GOOD: Reuse pre-allocated RectF
        rectF.set(10f, 10f, width - 10f, height - 10f)

        // GOOD: Reset and reuse Path
        path.reset()
        path.moveTo(0f, 0f)
        path.lineTo(width.toFloat(), height.toFloat())

        // GOOD: Reuse Matrix
        matrix.reset()
        matrix.setRotate(45f)

        // GOOD: Reuse StringBuilder
        textBuilder.clear()
        textBuilder.append("Size: ")
        textBuilder.append(width)
        textBuilder.append("x")
        textBuilder.append(height)

        canvas.drawRect(rect, fillPaint)
        canvas.drawPath(path, strokePaint)
        canvas.drawText(textBuilder.toString(), 100f, 100f, fillPaint)
    }

    // GOOD: Create gradient in onSizeChanged (called rarely)
    private var gradient: LinearGradient? = null

    override fun onSizeChanged(w: Int, h: Int, oldw: Int, oldh: Int) {
        super.onSizeChanged(w, h, oldw, oldh)

        // OK: onSizeChanged is called rarely
        gradient = LinearGradient(
            0f, 0f, w.toFloat(), h.toFloat(),
            Color.RED, Color.BLUE, Shader.TileMode.CLAMP
        )
        fillPaint.shader = gradient
    }
}

// GOOD: Lazy initialization outside onDraw
class LazyInitView(context: Context) : View(context) {

    private val paint by lazy {
        Paint().apply {
            color = Color.RED
            isAntiAlias = true
        }
    }

    private val path by lazy { Path() }

    override fun onDraw(canvas: Canvas) {
        super.onDraw(canvas)
        // GOOD: Objects initialized lazily, not on every frame
        canvas.drawPath(path, paint)
    }
}

// Supporting types
abstract class ViewGroup(context: Context, attrs: AttributeSet?) : View(context, attrs, 0) {
    abstract fun onLayout(changed: Boolean, l: Int, t: Int, r: Int, b: Int)
    open fun dispatchDraw(canvas: Canvas) {}
}
