# Rock

## Usage

Config file:
```json
{
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
            }
        }
    ]
}
```
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
