using Cheetah.Matches.Realtime.Codec;
using Cheetah.Matches.Realtime.Codec.Formatter;
using Cheetah.Matches.Realtime.Types;
using UnityEngine;
using Cheetah.Matches.Realtime.Tests.Codec.Field;

// ReSharper disable once CheckNamespace
namespace Cheetah.Matches.Realtime.Tests.Codec.Field
{
		// warning warning warning warning warning
		// Code generated by Cheetah relay codec generator - DO NOT EDIT
		// warning warning warning warning warning
		public class TestFormatterReferencedArrayFieldStructureCodec:Codec<Cheetah.Matches.Realtime.Tests.Codec.Field.TestFormatterReferencedArrayField.Structure>
		{
			public void Decode(ref CheetahBuffer buffer, ref Cheetah.Matches.Realtime.Tests.Codec.Field.TestFormatterReferencedArrayField.Structure dest)
			{
				dest.size = UIntFormatter.Instance.Read(ref buffer);
				StringFormatter.Instance.ReadArray(ref buffer, dest.stringArray, dest.size,0);
				VariableSizeIntFormatter.Instance.ReadArray(ref buffer, dest.variableSizeArray, dest.size,0);
			}
	
			public void  Encode(ref Cheetah.Matches.Realtime.Tests.Codec.Field.TestFormatterReferencedArrayField.Structure source, ref CheetahBuffer buffer)
			{
				UIntFormatter.Instance.Write(source.size,ref buffer);
				StringFormatter.Instance.WriteArray(source.stringArray,source.size, 0,ref buffer);
				VariableSizeIntFormatter.Instance.WriteArray(source.variableSizeArray,source.size, 0,ref buffer);
			}
	
	
			[RuntimeInitializeOnLoadMethod(RuntimeInitializeLoadType.SubsystemRegistration)]
			private static void OnRuntimeMethodLoad()
			{
				CodecRegistryBuilder.RegisterDefault(factory=>new TestFormatterReferencedArrayFieldStructureCodec());
			}
	
		}
	
	
}
