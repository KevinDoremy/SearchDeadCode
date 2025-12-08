package com.example.antipattern

// BAD: Excessive scope function chaining
class ScopeFunctionAbuse {

    fun processUser(user: User?) {
        // Anti-pattern: chaining multiple scope functions
        user?.let { u ->
            u.apply {
                name = "Updated"
            }.also { updated ->
                log(updated)
            }.run {
                save()
            }
        }
    }

    fun confusingChain(data: Data?) {
        // Anti-pattern: nested lets creating "pyramid of doom"
        data?.let { d ->
            d.field?.let { f ->
                f.nested?.let { n ->
                    n.value?.let { v ->
                        process(v)
                    }
                }
            }
        }
    }

    // GOOD: Simple scope function usage
    fun simpleUsage(user: User?) {
        user?.let { saveUser(it) }
    }

    // GOOD: Single apply for initialization
    fun createConfig(): Config {
        return Config().apply {
            timeout = 30
            retries = 3
        }
    }

    // BAD: Using with() on nullable receiver
    fun badWith(user: User?) {
        with(user!!) {  // Force unwrap is dangerous
            println(name)
        }
    }

    // GOOD: Using ?.let for nullable
    fun goodNullable(user: User?) {
        user?.let {
            println(it.name)
        }
    }
}
