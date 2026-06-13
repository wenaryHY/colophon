-- POST 请求脚本 for wrk
-- 用法: wrk -t4 -c50 -d15s --latency -s benches/post_script.lua http://localhost:3000/api/admin/posts

wrk.method = "POST"
wrk.headers["Content-Type"] = "application/json"

-- 替换为实际的 JWT token
-- 运行前设置环境变量: export COLOPHON_TOKEN="your_jwt_token"
local token = os.getenv("COLOPHON_TOKEN") or "REPLACE_WITH_YOUR_TOKEN"
wrk.headers["Authorization"] = "Bearer " .. token

counter = 0

request = function()
    counter = counter + 1
    
    local body = string.format([[{
        "title": "Benchmark Post %d",
        "slug": "benchmark-post-%d",
        "content": "This is a benchmark post created for performance testing. Lorem ipsum dolor sit amet, consectetur adipiscing elit.",
        "status": "draft",
        "visibility": "public"
    }]], counter, counter)
    
    return wrk.format(nil, nil, nil, body)
end

response = function(status, headers, body)
    if status ~= 200 and status ~= 201 then
        print("Error response: " .. status)
        print("Body: " .. body)
    end
end
