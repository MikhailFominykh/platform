using System;
using Cheetah.Matches.Realtime.Codec;

namespace Cheetah.Matches.Realtime.Editor.Generator.Fields.Array.Exceptions
{
    internal class VariableSizeTypeNotSupportedException : Exception
    {
        public VariableSizeTypeNotSupportedException(Type type) : base("Type " + type.Name + " not supported with " + nameof(VariableSizeCodec))
        {
        }
    }
}