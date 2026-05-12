Shader "e2d/Curve" {
	Properties {
		_ControlSize("Control Size", Float) = 1
		_InvControlSize("Inv Control Size", Float) = 1
		_InvControlSizeHalf("Half of Inv Control Size", Float) = 0.5
		_Control("Control (RGBA)", 2D) = "red" {}
		_Splat0("Layer 0 (R)", 2D) = "white" {}
		_SplatParams0("Splat Params 0", Vector) = (1,1,0,0)
		_Splat1("Layer 1 (G)", 2D) = "white" {}
		_SplatParams1("Splat Params 1", Vector) = (1,1,0,0)
		_MainTex("BaseMap (RGB)", 2D) = "white" {}
		_Color("Main Color", Color) = (1,1,1,1)
	}
	SubShader{
		Tags { "QUEUE" = "Transparent" "IGNOREPROJECTOR" = "False" "RenderType" = "Opaque" }
		Pass {
			Tags { "QUEUE" = "Transparent" "IGNOREPROJECTOR" = "False" "RenderType" = "Opaque" }
			Blend SrcAlpha OneMinusSrcAlpha
			CGPROGRAM
			#pragma vertex vert
			#pragma fragment frag
			#include "UnityCG.cginc"
			struct appdata {
				fixed4 color : COLOR;
				float4 vertex : POSITION;
				float2 texcoord : TEXCOORD0;
			};
			struct v2f {
				float4 vertex : SV_POSITION;
				float2 texcoord0 : TEXCOORD0;
				float2 texcoord1 : TEXTCOORD1;
			};
			float _InvControlSize;
			float _InvControlSizeHalf;
			float4 _SplatParams0;
			v2f vert(appdata v)
			{
				v2f o;
				o.vertex = UnityObjectToClipPos(v.vertex);
				o.texcoord0 = float2(v.texcoord.y* _InvControlSize + _InvControlSizeHalf,0.0);
				o.texcoord1 = float2(v.texcoord.x* _SplatParams0.x, v.color.x);
				return o;
			}
			sampler2D _Control;
			sampler2D _Splat0;
			sampler2D _Splat1;
			fixed4 frag(v2f i) : SV_Target
			{
				half4 var = tex2D(_Splat1, i.texcoord1);
				half var2 = floor(tex2D(_Control, i.texcoord0).y);
				fixed4 var3 = tex2D(_Splat0, i.texcoord1);
				var3.xyz += (var.xyz - var3.xyz) * var2;
				return var3;
			}
			ENDCG
		}
	}
	Fallback "VertexLit"
}