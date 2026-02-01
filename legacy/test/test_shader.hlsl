struct type_6 {
    row_major float4x4 member;
};

static float4 global = (float4)0;
static float2 global_1 = (float2)0;
static float4 global_2 = float4(0.0, 0.0, 0.0, 1.0);
cbuffer global_3 : register(b0) { type_6 global_3; }

void function()
{
    float4x4 _e6 = global_3.member;
    float4 _e7 = global;
    global_2 = mul(_e7, _e6);
    return;
}

float4 SimpleVertex(float4 param : LOC0, float2 param_1 : LOC1) : SV_Position
{
    global = param;
    global_1 = param_1;
    function();
    float _e6 = global_2.y;
    global_2.y = -(_e6);
    float4 _e8 = global_2;
    return _e8;
}
