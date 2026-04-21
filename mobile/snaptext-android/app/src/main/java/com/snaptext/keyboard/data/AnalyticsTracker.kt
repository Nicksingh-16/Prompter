package com.snaptext.keyboard.data

import android.content.Context
import org.json.JSONObject
import java.text.SimpleDateFormat
import java.util.*

/**
 * Tracks communication analytics — transforms, tone trends, streaks.
 * Powers the Analytics Widget and weekly digest.
 */
class AnalyticsTracker(context: Context) {

    private val prefs = context.getSharedPreferences("snaptext_analytics", Context.MODE_PRIVATE)
    private val dateFormat = SimpleDateFormat("yyyy-MM-dd", Locale.US)

    data class DailyStats(
        val date: String,
        val transforms: Int,
        val avgToneScore: Float,
        val modesUsed: Map<String, Int>
    )

    data class WeeklySummary(
        val totalTransforms: Int,
        val currentStreak: Int,
        val topMode: String,
        val avgToneScore: Float,
        val toneImprovement: Float,  // vs previous week
        val dailyStats: List<DailyStats>
    )

    // ── Record events ───────────────────────────────────────────────────────

    fun recordTransform(mode: String, toneScore: Float) {
        val today = dateFormat.format(Date())
        val dayData = getDayData(today)

        val transforms = dayData.optInt("transforms", 0) + 1
        val totalTone = dayData.optDouble("totalTone", 0.0) + toneScore
        val modes = dayData.optJSONObject("modes") ?: JSONObject()
        modes.put(mode, modes.optInt(mode, 0) + 1)

        dayData.put("transforms", transforms)
        dayData.put("totalTone", totalTone)
        dayData.put("modes", modes)

        saveDayData(today, dayData)
        updateStreak(today)
    }

    // ── Query stats ─────────────────────────────────────────────────────────

    fun getTodayStats(): DailyStats {
        val today = dateFormat.format(Date())
        return parseDayStats(today)
    }

    fun getWeeklySummary(): WeeklySummary {
        val cal = Calendar.getInstance()
        val thisWeek = mutableListOf<DailyStats>()

        for (i in 6 downTo 0) {
            cal.time = Date()
            cal.add(Calendar.DAY_OF_YEAR, -i)
            thisWeek.add(parseDayStats(dateFormat.format(cal.time)))
        }

        val totalTransforms = thisWeek.sumOf { it.transforms }
        val avgTone = if (totalTransforms > 0) {
            thisWeek.filter { it.transforms > 0 }.map { it.avgToneScore }.average().toFloat()
        } else 0f

        // Calculate last week's average for comparison
        val lastWeekTones = mutableListOf<Float>()
        for (i in 13 downTo 7) {
            cal.time = Date()
            cal.add(Calendar.DAY_OF_YEAR, -i)
            val stats = parseDayStats(dateFormat.format(cal.time))
            if (stats.transforms > 0) lastWeekTones.add(stats.avgToneScore)
        }
        val lastWeekAvg = if (lastWeekTones.isNotEmpty()) lastWeekTones.average().toFloat() else 0f
        val improvement = avgTone - lastWeekAvg

        // Top mode
        val allModes = mutableMapOf<String, Int>()
        thisWeek.forEach { day -> day.modesUsed.forEach { (mode, count) -> allModes[mode] = (allModes[mode] ?: 0) + count } }
        val topMode = allModes.maxByOrNull { it.value }?.key ?: "None"

        return WeeklySummary(
            totalTransforms = totalTransforms,
            currentStreak = getStreak(),
            topMode = topMode,
            avgToneScore = avgTone,
            toneImprovement = improvement,
            dailyStats = thisWeek
        )
    }

    fun getStreak(): Int = prefs.getInt("streak", 0)

    fun getTotalTransforms(): Int = prefs.getInt("total_transforms", 0)

    // ── Internal ────────────────────────────────────────────────────────────

    private fun parseDayStats(date: String): DailyStats {
        val dayData = getDayData(date)
        val transforms = dayData.optInt("transforms", 0)
        val totalTone = dayData.optDouble("totalTone", 0.0).toFloat()
        val avgTone = if (transforms > 0) totalTone / transforms else 0f
        val modesJson = dayData.optJSONObject("modes")
        val modes = mutableMapOf<String, Int>()
        modesJson?.keys()?.forEach { key -> modes[key] = modesJson.optInt(key, 0) }

        return DailyStats(date, transforms, avgTone, modes)
    }

    private fun getDayData(date: String): JSONObject {
        val json = prefs.getString("day_$date", null) ?: return JSONObject()
        return try { JSONObject(json) } catch (_: Exception) { JSONObject() }
    }

    private fun saveDayData(date: String, data: JSONObject) {
        prefs.edit().putString("day_$date", data.toString()).apply()
        // Update total
        val total = prefs.getInt("total_transforms", 0) + 1
        prefs.edit().putInt("total_transforms", total).apply()
    }

    private fun updateStreak(today: String) {
        val lastActiveDate = prefs.getString("last_active_date", null)
        val currentStreak = prefs.getInt("streak", 0)

        if (lastActiveDate == null || lastActiveDate != today) {
            val cal = Calendar.getInstance()
            cal.add(Calendar.DAY_OF_YEAR, -1)
            val yesterday = dateFormat.format(cal.time)

            val newStreak = if (lastActiveDate == yesterday) currentStreak + 1 else 1
            prefs.edit()
                .putString("last_active_date", today)
                .putInt("streak", newStreak)
                .apply()
        }
    }
}
