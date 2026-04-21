package com.snaptext.keyboard.data

import android.content.Context
import org.json.JSONArray
import org.json.JSONObject

/**
 * Quick Templates — parameterized message templates with categories.
 * Templates use {placeholder} syntax for fill-in fields.
 */
class TemplateManager(context: Context) {

    data class Template(
        val id: String,
        val name: String,
        val body: String,
        val category: String,
        val isBuiltIn: Boolean = false,
        val usageCount: Int = 0
    ) {
        /** Extract placeholder names like {name}, {date} */
        fun getPlaceholders(): List<String> {
            return Regex("\\{(\\w+)}").findAll(body).map { it.groupValues[1] }.toList()
        }

        /** Fill placeholders with values */
        fun fill(values: Map<String, String>): String {
            var result = body
            for ((key, value) in values) {
                result = result.replace("{$key}", value)
            }
            return result
        }
    }

    private val prefs = context.getSharedPreferences("snaptext_templates", Context.MODE_PRIVATE)
    private val templates = mutableListOf<Template>()

    init {
        loadTemplates()
        if (templates.isEmpty()) {
            seedDefaults()
        }
    }

    fun getAll(): List<Template> = templates.sortedByDescending { it.usageCount }

    fun getByCategory(category: String): List<Template> =
        templates.filter { it.category == category }.sortedByDescending { it.usageCount }

    fun getCategories(): List<String> = templates.map { it.category }.distinct()

    fun addTemplate(name: String, body: String, category: String): Template {
        val template = Template(
            id = "user_${System.currentTimeMillis()}",
            name = name,
            body = body,
            category = category,
            isBuiltIn = false
        )
        templates.add(template)
        save()
        return template
    }

    fun removeTemplate(id: String) {
        templates.removeAll { it.id == id && !it.isBuiltIn }
        save()
    }

    fun incrementUsage(id: String) {
        val index = templates.indexOfFirst { it.id == id }
        if (index >= 0) {
            templates[index] = templates[index].copy(usageCount = templates[index].usageCount + 1)
            save()
        }
    }

    // ── Default templates ───────────────────────────────────────────────────

    private fun seedDefaults() {
        val defaults = listOf(
            // Work
            Template("w1", "Meeting Confirmation", "Hey {name}, confirming our meeting on {date} at {time}. Let me know if this still works.", "Work", true),
            Template("w2", "Quick Follow-Up", "Hi {name}, just following up on our conversation about {topic}. Any updates on your end?", "Work", true),
            Template("w3", "Leave Request", "Hi {name}, I'd like to request leave from {start_date} to {end_date} for {reason}. I'll ensure all pending work is handed over.", "Work", true),
            Template("w4", "Task Update", "Hi {name}, quick update on {task}: {status}. Let me know if you need anything else.", "Work", true),
            Template("w5", "Deadline Extension", "Hi {name}, could I get an extension on {task}? Current deadline is {date}, and I'd need until {new_date} because {reason}.", "Work", true),

            // Social
            Template("s1", "Birthday Wish", "Happy birthday, {name}! Wishing you an amazing year ahead. Hope your day is as awesome as you are!", "Social", true),
            Template("s2", "Thank You", "Hey {name}, just wanted to say thank you for {reason}. Really appreciate it!", "Social", true),
            Template("s3", "Plans", "Hey {name}! Are you free on {day}? Was thinking we could {activity}.", "Social", true),
            Template("s4", "Congratulations", "Congratulations, {name}! So happy to hear about {achievement}. Well deserved!", "Social", true),

            // Errands
            Template("e1", "Appointment", "Hi, I'd like to schedule an appointment for {date} at {time}. My name is {name}, and my contact number is {phone}.", "Errands", true),
            Template("e2", "Order Inquiry", "Hi, I placed an order on {date}, order ID: {order_id}. Could you provide an update on the delivery status?", "Errands", true),
            Template("e3", "Complaint", "Hi, I'm writing regarding {issue} that occurred on {date}. {details}. I'd appreciate a resolution at the earliest.", "Errands", true),

            // Follow-ups
            Template("f1", "Gentle Reminder", "Hi {name}, just a gentle reminder about {topic}. Let me know if you need any additional info from my end.", "Follow-ups", true),
            Template("f2", "Payment Reminder", "Hi {name}, this is a friendly reminder about the pending payment of {amount} for {item}. Please let me know once it's done.", "Follow-ups", true),
            Template("f3", "Application Status", "Hi {name}, I'm writing to follow up on my application for {position} submitted on {date}. I'd love to know if there are any updates.", "Follow-ups", true),
        )

        templates.addAll(defaults)
        save()
    }

    // ── Persistence ─────────────────────────────────────────────────────────

    private fun save() {
        val arr = JSONArray()
        for (t in templates) {
            arr.put(JSONObject().apply {
                put("id", t.id)
                put("name", t.name)
                put("body", t.body)
                put("category", t.category)
                put("builtIn", t.isBuiltIn)
                put("usageCount", t.usageCount)
            })
        }
        prefs.edit().putString("templates", arr.toString()).apply()
    }

    private fun loadTemplates() {
        val json = prefs.getString("templates", null) ?: return
        try {
            val arr = JSONArray(json)
            templates.clear()
            for (i in 0 until arr.length()) {
                val obj = arr.getJSONObject(i)
                templates.add(Template(
                    id = obj.getString("id"),
                    name = obj.getString("name"),
                    body = obj.getString("body"),
                    category = obj.getString("category"),
                    isBuiltIn = obj.optBoolean("builtIn", false),
                    usageCount = obj.optInt("usageCount", 0)
                ))
            }
        } catch (_: Exception) {
            // Start fresh on corruption
        }
    }
}
