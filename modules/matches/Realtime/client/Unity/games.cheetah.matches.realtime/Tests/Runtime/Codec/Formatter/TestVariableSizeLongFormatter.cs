using Cheetah.Matches.Realtime.Codec.Formatter;

namespace Cheetah.Matches.Realtime.Tests.Codec.Formatter
{
    public class TestVariableSizeLongFormatter : AbstractVariableSizeFormatterTest<long, VariableSizeLongFormatter>
    {
        protected override long[] GetValues()
        {
            return new[]
            {
                -120,
                short.MinValue,
                short.MaxValue,
                int.MinValue,
                int.MaxValue,
                long.MinValue,
                long.MaxValue
            };
        }

        protected override int[] GetSizes()
        {
            return new[]
            {
                1, 3, 3, 5, 5, 9, 9
            };
        }
    }
}