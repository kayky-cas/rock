# Rock

## TODOs

- [ ] Extract path params to use them

## Usage

Config file:
```json
{
    "delay": 500,
    "proxy": {
        "host": "google.com",
        "port": 443 // or 80 to http
    },
    "responses": [
        {
            "path": "/",
            "method": "GET",
            "enabled": true,
            "status": 200,
            "body": {
                "message": "hello"
            }
        },
        {
            "path": "/hello/{a}",
            "method": "GET",
            "enabled": true,
            "status": 200,
            "body": {
                "message": "hello to {a}"
            },
            "delay": 100
        }
    ]
}
```

## Delay

You can add a `delay` field (in milliseconds) to simulate response latency.

- **Global delay**: Set `delay` at the config root to apply to all responses (both mock and proxied).
- **Per-request delay**: Set `delay` on an individual response to override the global delay for that route.
- If neither is set, responses are served immediately.

```json
{
    "delay": 500,
    "proxy": { "host": "example.com", "port": 443 },
    "responses": [
        {
            "path": "/fast",
            "method": "GET",
            "status": 200,
            "body": "quick",
            "delay": 50
        }
    ]
}
```

In this example, `/fast` responds after 50ms (per-request override), while all other routes (including proxied ones) are delayed by 500ms.
```bash
$ rock -p 3000 -f config.json
```
```bash
$ curl localhost:3000
{"message":"hello"}
```
```bash
$ curl localhost:3000/hello/kayky
{"message":"hello to kayky"}
```
```bash
$ curl localhost:3000/images
# will be proxied to google.com:443/images
```
