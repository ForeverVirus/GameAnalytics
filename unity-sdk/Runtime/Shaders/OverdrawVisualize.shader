// Overdraw Visualization Shader for GameAnalytics Profiler
// Each rendered fragment adds a fixed amount to the red channel.
// Result: higher red = more overdraw layers.

Shader "Hidden/GAProfiler/Overdraw"
{
    SubShader
    {
        Tags { "RenderType" = "Opaque" }

        // Additive blending: each draw adds to existing color
        Blend One One
        ZTest LEqual
        ZWrite Off
        Cull Off

        Pass
        {
            CGPROGRAM
            #pragma vertex vert
            #pragma fragment frag

            #include "UnityCG.cginc"

            struct appdata
            {
                float4 vertex : POSITION;
            };

            struct v2f
            {
                float4 pos : SV_POSITION;
            };

            v2f vert(appdata v)
            {
                v2f o;
                o.pos = UnityObjectToClipPos(v.vertex);
                return o;
            }

            fixed4 frag(v2f i) : SV_Target
            {
                // Each draw adds 1/255 ≈ 0.004 to the red channel.
                // After readback, red * 255 = number of overdraw layers.
                return fixed4(1.0 / 255.0, 0, 0, 0);
            }
            ENDCG
        }
    }

    // Transparent objects
    SubShader
    {
        Tags { "RenderType" = "Transparent" }

        Blend One One
        ZTest LEqual
        ZWrite Off
        Cull Off

        Pass
        {
            CGPROGRAM
            #pragma vertex vert
            #pragma fragment frag

            #include "UnityCG.cginc"

            struct appdata
            {
                float4 vertex : POSITION;
            };

            struct v2f
            {
                float4 pos : SV_POSITION;
            };

            v2f vert(appdata v)
            {
                v2f o;
                o.pos = UnityObjectToClipPos(v.vertex);
                return o;
            }

            fixed4 frag(v2f i) : SV_Target
            {
                return fixed4(1.0 / 255.0, 0, 0, 0);
            }
            ENDCG
        }
    }

    // Fallback for any other RenderType
    SubShader
    {
        Tags { "RenderType" = "" }

        Blend One One
        ZTest LEqual
        ZWrite Off

        Pass
        {
            CGPROGRAM
            #pragma vertex vert
            #pragma fragment frag

            #include "UnityCG.cginc"

            struct appdata
            {
                float4 vertex : POSITION;
            };

            struct v2f
            {
                float4 pos : SV_POSITION;
            };

            v2f vert(appdata v)
            {
                v2f o;
                o.pos = UnityObjectToClipPos(v.vertex);
                return o;
            }

            fixed4 frag(v2f i) : SV_Target
            {
                return fixed4(1.0 / 255.0, 0, 0, 0);
            }
            ENDCG
        }
    }
}
