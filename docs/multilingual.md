# Multi-Language Support

Seen's keywords are defined in external TOML files, enabling programming in 6 languages with no compiler changes.

## How It Works

Each language has a directory under `languages/` containing TOML files for keywords, operators, and standard library names. The lexer reads these at compile time to tokenize source in the specified language.

```bash
seen build hello.seen --language ar    # Compile Arabic source
seen build hello.seen --language es    # Compile Spanish source
seen build hello.seen -l zh            # Compile Chinese source
```

The `language` field in `Seen.toml` sets the default:

```toml
[project]
name = "my_project"
language = "ar"
```

## Hello World in All 6 Languages

### English (en)

```seen
fun main() {
    println("Hello, World!")
}
```

### Arabic (ar)

```seen
دالة main() {
    println("!مرحبا بالعالم")
}
```

### Spanish (es)

```seen
función main() {
    println("Hola, Mundo!")
}
```

### Russian (ru)

```seen
функция main() {
    println("Привет, мир!")
}
```

### Chinese (zh)

```seen
函数 main() {
    println("你好，世界！")
}
```

### Japanese (ja)

```seen
関数 main() {
    println("こんにちは、世界！")
}
```

## Complete Keyword Translation Tables

### Control Flow (20 keywords)

| English | Arabic | Spanish | Russian | Chinese | Japanese |
|---------|--------|---------|---------|---------|----------|
| `fun` | `دالة` | `función` | `функция` | `函数` | `関数` |
| `if` | `إذا` | `si` | `если` | `如果` | `もし` |
| `else` | `وإلا` | `sino` | `иначе` | `否则` | `そうでなければ` |
| `while` | `بينما` | `mientras` | `пока` | `当` | `間` |
| `for` | `لكل` | `para` | `для` | `对于` | `のために` |
| `in` | `في` | `en` | `в` | `在` | `で` |
| `match` | `طابق` | `coincidir` | `совпадение` | `匹配` | `一致` |
| `break` | `اكسر` | `romper` | `прервать` | `中断` | `中止` |
| `continue` | `استمر` | `continuar` | `продолжить` | `继续` | `継続` |
| `return` | `رجع` | `retornar` | `вернуть` | `返回` | `戻る` |
| `when` | `عندما` | `cuando` | `когда` | `当时` | `場合` |
| `try` | `جرب` | `intentar` | `попробовать` | `尝试` | `試行` |
| `catch` | `امسك` | `capturar` | `поймать` | `捕获` | `捕捉` |
| `finally` | `أخيرا` | `finalmente` | `наконец` | `最终` | `最後に` |
| `throw` | `ارم` | `lanzar_error` | `бросить` | `抛出` | `投げる` |
| `loop` | `حلقة` | `bucle` | `цикл` | `循环` | `繰返` |
| `defer` | `أجل` | `diferir` | `отложить` | `延迟` | `延期` |
| `assert` | `أكد` | `afirmar` | `утверждать` | `断言` | `表明` |
| `unsafe` | `غير_آمن` | `inseguro` | `небезопасный` | `不安全` | `安全でない` |
| `errdefer` | `أجل_خطأ` | `diferir_error` | `отложить_ошибку` | `错误延迟` | `エラー延期` |

### Type Definitions (21 keywords)

| English | Arabic | Spanish | Russian | Chinese | Japanese |
|---------|--------|---------|---------|---------|----------|
| `class` | `فئة` | `clase` | `класс` | `类` | `クラス` |
| `struct` | `هيكل` | `estructura` | `структура` | `结构` | `構造体` |
| `enum` | `تعداد` | `enumeración` | `перечисление` | `枚举` | `列挙` |
| `data` | `بيانات` | `datos` | `данные` | `数据` | `データ` |
| `trait` | `سمة` | `rasgo_tipo` | `трейт` | `特征` | `トレイト` |
| `impl` | `تطبيق` | `implementar` | `реализовать` | `实现` | `実装` |
| `type` | `نوع` | `tipo` | `тип` | `类型` | `型` |
| `interface` | `واجهة` | `interfaz` | `интерфейс` | `接口` | `インターフェース` |
| `extends` | `يمتد` | `extiende` | `расширяет` | `继承` | `継承` |
| `sealed` | `مختوم` | `sellado` | `запечатанный` | `密封` | `封印` |
| `object` | `كائن` | `objeto` | `объект` | `对象` | `オブジェクト` |
| `abstract` | `مجرد` | `abstracto` | `абстрактный` | `抽象` | `抽象` |
| `final` | `نهائي` | `final` | `финальный` | `最终的` | `最終` |
| `override` | `تجاوز` | `anular` | `переопределить` | `重写` | `上書き` |
| `open` | `مفتوح` | `abierto` | `открытый` | `开放` | `開放` |
| `extension` | `امتداد` | `extensión` | `расширение` | `扩展` | `拡張` |
| `companion` | `رفيق` | `compañero` | `компаньон` | `伴生` | `同伴` |
| `spec` | `صفة` | `rasgo` | `черта` | `规范` | `仕様` |
| `distinct` | `مميز` | `distinto` | `различный` | `独特` | `独自` |
| `union` | `اتحاد` | `unión` | `объединение` | `联合` | `共用体` |
| `lateinit` | `متأخر_البدء` | `init_tardío` | `поздняя_инициализация` | `延迟初始化` | `遅延初期化` |

### Variable Declarations (6 keywords)

| English | Arabic | Spanish | Russian | Chinese | Japanese |
|---------|--------|---------|---------|---------|----------|
| `let` | `اجعل` | `sea` | `пусть` | `让` | `定数` |
| `var` | `متغير` | `variable` | `переменная` | `变量` | `変数` |
| `val` | `ثابت` | `valor` | `значение` | `值` | `値` |
| `mut` | `قابل_للتغيير` | `mutable` | `изменяемый` | `可变` | `可変` |
| `const` | `ثابت_عام` | `constante` | `константа` | `常量` | `定義` |
| `static` | `ساكن` | `estático` | `статический` | `静态` | `静的` |

### Module System (4 keywords)

| English | Arabic | Spanish | Russian | Chinese | Japanese |
|---------|--------|---------|---------|---------|----------|
| `module` | `وحدة` | `módulo` | `модуль` | `模块` | `モジュール` |
| `import` | `استورد` | `importar` | `импортировать` | `导入` | `インポート` |
| `use` | `استخدم` | `usar` | `использовать` | `使用` | `使用` |
| `external` | `خارجي` | `externo` | `внешний` | `外部` | `外部` |

### Literals (3 keywords)

| English | Arabic | Spanish | Russian | Chinese | Japanese |
|---------|--------|---------|---------|---------|----------|
| `true` | `صحيح` | `verdadero` | `истина` | `真` | `真` |
| `false` | `خطأ` | `falso` | `ложь` | `假` | `偽` |
| `null` | `عدم` | `nulo` | `ноль` | `空` | `空` |

### Async/Concurrency (19 keywords)

| English | Arabic | Spanish | Russian | Chinese | Japanese |
|---------|--------|---------|---------|---------|----------|
| `async` | `غير_متزامن` | `asíncrono` | `асинхронный` | `异步` | `非同期` |
| `await` | `انتظر` | `esperar` | `ждать` | `等待` | `待機` |
| `spawn` | `ولد` | `generar` | `породить` | `生成` | `生成` |
| `select` | `اختر` | `seleccionar` | `выбрать` | `选择` | `選択` |
| `actor` | `ممثل` | `actor` | `актёр` | `角色` | `アクター` |
| `receive` | `استقبل` | `recibir` | `получить` | `接收` | `受信` |
| `send` | `أرسل` | `enviar` | `отправить` | `发送` | `送信` |
| `parallel_for` | `parallel_for` | `parallel_for` | `parallel_for` | `并行遍历` | `並列反復` |
| `scope` | `نطاق` | `ambito` | `scope` | `域` | `スコープ` |
| `cancel` | `الغاء` | `cancelar` | `cancel` | `取消` | `キャンセル` |
| `detached` | `منفصل` | `desvinculado` | `detached` | `分离` | `切離` |
| `suspend` | `علق` | `suspender` | `приостановить` | `挂起` | `中断` |
| `launch` | `شغل` | `lanzar` | `запустить` | `启动` | `起動` |
| `flow` | `تدفق` | `flujo` | `поток` | `流` | `フロー` |
| `emit` | `اصدر` | `emitir` | `излучить` | `发射` | `放出` |
| `delay` | `أخر` | `retrasar` | `задержать` | `延时` | `遅延` |
| `request` | `طلب` | `solicitar` | `запросить` | `请求` | `リクエスト` |
| `reply` | `رد` | `responder` | `ответить` | `回复` | `返信` |
| `observable` | `قابل_المراقبة` | `observable` | `наблюдаемый` | `可观察` | `観察可能` |

### Memory Management (9 keywords)

| English | Arabic | Spanish | Russian | Chinese | Japanese |
|---------|--------|---------|---------|---------|----------|
| `move` | `انقل` | `mover` | `переместить` | `移动` | `移動` |
| `borrow` | `استعر` | `prestar` | `заимствовать` | `借用` | `借用` |
| `ref` | `مرجع` | `referencia` | `ссылка` | `引用` | `参照` |
| `own` | `ملك` | `propio` | `владение` | `拥有` | `所有` |
| `inout` | `في_المكان` | `en_lugar` | `на_месте` | `输入输出` | `入出力` |
| `region` | `منطقة` | `región` | `регион` | `区域` | `領域` |
| `arena` | `حلبة` | `arena` | `арена` | `竞技场` | `アリーナ` |
| `transmute` | `تحويل` | `transmutar` | `трансмутировать` | `变换` | `変換` |
| `gpu` | `معالج_رسومات` | `gpu` | `гпу` | `图形处理` | `グラフィック` |

### Metaprogramming (2 keywords)

| English | Arabic | Spanish | Russian | Chinese | Japanese |
|---------|--------|---------|---------|---------|----------|
| `comptime` | `وقت_التجميع` | `tiempo_compilación` | `время_компиляции` | `编译时` | `コンパイル時` |
| `macro` | `ماكرو` | `macro` | `макрос` | `宏` | `マクロ` |

### Miscellaneous (9 keywords)

| English | Arabic | Spanish | Russian | Chinese | Japanese |
|---------|--------|---------|---------|---------|----------|
| `is` | `هو` | `es` | `является` | `是` | `である` |
| `as` | `ك` | `como` | `как` | `作为` | `として` |
| `and` | `و` | `y` | `и` | `和` | `且つ` |
| `or` | `أو` | `o` | `или` | `或` | `又は` |
| `not` | `ليس` | `no` | `не` | `不` | `非` |
| `by` | `بواسطة` | `por` | `через` | `由` | `により` |
| `to` | `إلى` | `a` | `к` | `到` | `まで` |
| `from` | `من` | `de` | `от` | `从` | `から` |
| `effect` | `تأثير` | `efecto` | `эффект` | `效果` | `効果` |

## Return Type Label

The return type label `r:` has an Arabic alternative:

| Language | Syntax |
|----------|--------|
| All languages | `r:` |
| Arabic (alternative) | `ن:` |

Example:

```seen
// English
fun add(a: Int, b: Int) r: Int { return a + b }

// Arabic
دالة add(a: Int, b: Int) ن: Int { رجع a + b }
```

## Operators

All operators use universal symbols across all languages:

```
+  -  *  /  %  =  ==  !=  <  <=  >  >=
+=  -=  *=  /=  %=  &  |  ^  ~  <<  >>
->  =>  ?  .  ..  ...  ..<  ::  ?.  ?:  !!  _  @
```

## Adding a New Language

1. Create `languages/xx/` (where `xx` is the ISO language code)
2. Copy the English TOML files as templates:
   ```bash
   cp -r languages/en/ languages/xx/
   ```
3. Translate keyword values in each TOML file
4. The compiler auto-detects available languages

No compiler rebuild required.

### TOML File Structure

Each language directory contains:
- `xx-keywords-control.toml` -- control flow keywords
- `xx-keywords-types.toml` -- type definition keywords
- `xx-keywords-vars.toml` -- variable declaration keywords
- `xx-keywords-module.toml` -- module system keywords
- `xx-keywords-literals.toml` -- literal keywords
- `xx-keywords-async.toml` -- async/concurrency keywords
- `xx-keywords-modifiers.toml` -- modifier keywords
- `xx-keywords-memory.toml` -- memory management keywords
- `xx-keywords-meta.toml` -- metaprogramming keywords
- `xx-keywords-misc.toml` -- miscellaneous keywords
- `xx-operators.toml` -- operator definitions

TOML format:

```toml
[keywords]
fun = "KeywordFun"
if = "KeywordIf"
else = "KeywordElse"
# ... etc
```

## File Structure

```
languages/
├── en/     # English (default)
├── ar/     # Arabic
├── es/     # Spanish
├── ru/     # Russian
├── zh/     # Chinese
└── ja/     # Japanese
```

Total: 11 files per language, 66 files across 6 languages.
