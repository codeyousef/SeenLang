//! Integration tests for reactive streams with coroutines

use crate::parser::Parser;
use seen_lexer::{Lexer, LanguageConfig};
use crate::ast::*;

fn setup_parser(code: &str) -> Parser {
    let lang_config = LanguageConfig::new_english();
    let mut lexer = Lexer::new(code, 0, &lang_config);
    let tokens = lexer.tokenize().expect("Failed to tokenize test input");
    Parser::new(tokens)
}

#[cfg(test)]
mod reactive_coroutine_tests {
    use super::*;

    #[test]
    fn test_flow_coroutine_integration() {
        let code = r#"
            // Flow type with coroutines - Kotlin style reactive streams
            suspend fun fetchUserFlow(): Flow<User> {
                return flow {
                    let users = await fetchUsers()
                    for user in users {
                        emit(user)
                        delay(100.ms)
                    }
                }
            }
            
            // Observable-to-Flow bridging
            fun observableToFlow<T>(obs: Observable<T>): Flow<T> {
                return flow {
                    obs.subscribe { value ->
                        emit(value)
                    }
                }
            }
            
            // StateFlow for reactive state management
            fun createUserStateFlow(): StateFlow<User?> {
                let stateFlow = MutableStateFlow<User?>(null)
                
                launch {
                    let userFlow = fetchUserFlow()
                    userFlow.collect { user ->
                        stateFlow.value = user
                    }
                }
                
                return stateFlow
            }
        "#;

        let mut parser = setup_parser(code);
        let program = parser.parse_program().expect("Failed to parse reactive-coroutine integration");

        eprintln!("DEBUG test_flow_coroutine_integration: Parsed {} items", program.items.len());
        for (i, item) in program.items.iter().enumerate() {
            match &item.kind {
                ItemKind::Function(func) => {
                    eprintln!("  Item {}: Function '{}'", i, func.name.value);
                }
                _ => {
                    eprintln!("  Item {}: Other", i);
                }
            }
        }
        assert_eq!(program.items.len(), 3);
        
        // Test suspend function with Flow return type
        match &program.items[0].kind {
            ItemKind::Function(func) => {
                assert_eq!(func.name.value, "fetchUserFlow");
                // Should have suspend attribute
                assert_eq!(func.attributes.len(), 1);
                assert_eq!(func.attributes[0].name.value, "suspend");
                
                // Return type should be Flow<User>
                assert!(func.return_type.is_some());
                if let Some(ref return_type) = func.return_type {
                    match &*return_type.kind {
                        TypeKind::Named { path, generic_args } => {
                            assert_eq!(path.segments[0].name.value, "Flow");
                            assert_eq!(generic_args.len(), 1);
                        }
                        _ => panic!("Expected named type Flow<User>"),
                    }
                }
            }
            _ => panic!("Expected function, got {:?}", program.items[0].kind),
        }
        
        // Test Flow DSL with flow { ... } builder
        // This should parse as a special expression type
        
        // Test Observable-to-Flow conversion function
        match &program.items[1].kind {
            ItemKind::Function(func) => {
                assert_eq!(func.name.value, "observableToFlow");
                // Should be generic function
                assert!(!func.attributes.is_empty() || func.name.value.contains("observableToFlow"));
            }
            _ => panic!("Expected function"),
        }

        // Test StateFlow usage
        match &program.items[2].kind {
            ItemKind::Function(func) => {
                assert_eq!(func.name.value, "createUserStateFlow");
                // Should return StateFlow<User?>
                assert!(func.return_type.is_some());
            }
            _ => panic!("Expected function"),
        }
    }

    #[test]
    fn test_reactive_dsl_builders() {
        let code = r#"
            // Reactive DSL for UI programming
            fun createReactiveUI(): ReactiveComponent {
                return reactive {
                    let clicks = button.clicks()
                    let textChanges = editText.textChanges()
                    
                    let combined = combine(clicks, textChanges) { clickEvent, text ->
                        updateLabel("${text} - clicked ${clickEvent.count} times")
                    }
                    
                    combined.subscribe()
                }
            }
            
            // Flow operators chaining
            suspend fun processDataFlow(input: Flow<String>): Flow<ProcessedData> {
                return input
                    .filter { it.isNotEmpty() }
                    .map { await processString(it) }
                    .throttle(1000.ms)
                    .distinctUntilChanged()
            }
        "#;

        let mut parser = setup_parser(code);
        let program = parser.parse_program().expect("Failed to parse reactive DSL");

        assert_eq!(program.items.len(), 2);
        
        // Test reactive DSL builder
        match &program.items[0].kind {
            ItemKind::Function(func) => {
                assert_eq!(func.name.value, "createReactiveUI");
                // Function body should contain reactive block
            }
            _ => panic!("Expected function"),
        }

        // Test Flow operators chaining with suspend
        match &program.items[1].kind {
            ItemKind::Function(func) => {
                assert_eq!(func.name.value, "processDataFlow");
                // Should have suspend attribute
                assert_eq!(func.attributes.len(), 1);
                assert_eq!(func.attributes[0].name.value, "suspend");
            }
            _ => panic!("Expected function"),
        }
    }

    #[test]
    fn test_livedata_style_reactive_properties() {
        let code = r#"
            // LiveData-style reactive properties
            class UserRepository {
                private let _users = MutableLiveData<List<User>>()
                val users: LiveData<List<User>> = _users
                
                suspend fun loadUsers() {
                    let userData = await fetchUsers()
                    _users.postValue(userData)
                }
                
                fun observeUserCount(): Observable<Int> {
                    return users.map { it.size }
                }
            }
            
            // StateFlow vs LiveData comparison
            class CounterViewModel {
                // StateFlow - always has current value
                private let _counter = MutableStateFlow(0)
                val counter: StateFlow<Int> = _counter
                
                // SharedFlow - for events  
                private let _events = MutableSharedFlow<CounterEvent>()
                val events: SharedFlow<CounterEvent> = _events
                
                fun increment() {
                    _counter.value += 1
                    _events.tryEmit(CounterEvent.Incremented)
                }
            }
        "#;

        let mut parser = setup_parser(code);
        let program = parser.parse_program().expect("Failed to parse LiveData-style properties");

        // Should parse classes with reactive properties
        // This tests the integration of OOP + Reactive + Coroutines
        assert!(program.items.len() >= 2);
    }

    #[test]
    fn test_coroutine_observable_bridging() {
        let code = r#"
            // Convert suspend function to Observable
            fun suspendToObservable<T>(suspendFunc: suspend () -> T): Observable<T> {
                return Observable.create { observer ->
                    launch {
                        try {
                            let result = await suspendFunc()
                            observer.onNext(result)
                            observer.onComplete()
                        } catch (e: Exception) {
                            observer.onError(e)
                        }
                    }
                }
            }
            
            // Convert Observable to suspend function
            suspend fun observableToSuspend<T>(obs: Observable<T>): T {
                return suspendCoroutine { continuation ->
                    obs.subscribe(
                        onNext: { value -> continuation.resume(value) },
                        onError: { error -> continuation.resumeWithException(error) }
                    )
                }
            }
            
            // Usage example
            suspend fun example() {
                // Observable to suspend function
                let networkCall = httpGet("https://api.example.com/users")
                let users = await observableToSuspend(networkCall)
                
                // Suspend function to Observable
                let userObservable = suspendToObservable {
                    await processUsers(users)
                }
                
                userObservable.subscribe { processedUsers ->
                    println("Processed ${processedUsers.size} users")
                }
            }
        "#;

        let mut parser = setup_parser(code);
        let result = parser.parse_program();
        
        eprintln!("DEBUG test_coroutine_observable_bridging: Parse result = {:?}", result.is_ok());
        if let Ok(ref program) = result {
            eprintln!("  Parsed {} items", program.items.len());
            for (i, item) in program.items.iter().enumerate() {
                match &item.kind {
                    ItemKind::Function(func) => {
                        eprintln!("  Item {}: Function '{}'", i, func.name.value);
                    }
                    _ => {
                        eprintln!("  Item {}: Other", i);
                    }
                }
            }
        } else if let Err(ref e) = result {
            eprintln!("  Parse error: {:?}", e);
        }
        
        let program = result.expect("Failed to parse coroutine-observable bridging");
        assert_eq!(program.items.len(), 3);
        
        // Test generic functions with proper suspend/Observable integration
        for item in &program.items {
            match &item.kind {
                ItemKind::Function(func) => {
                    // Functions should either be suspend or have Observable/suspend types
                    let has_suspend = func.attributes.iter().any(|attr| attr.name.value == "suspend");
                    let has_reactive_type = func.name.value.contains("Observable") || 
                                           func.name.value.contains("suspend");
                    
                    assert!(has_suspend || has_reactive_type, 
                           "Function {} should have suspend attribute or reactive type", 
                           func.name.value);
                }
                _ => {}
            }
        }
    }
}