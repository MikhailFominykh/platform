using System;
using Cheetah.Matches.Realtime.Internal;
using Cheetah.Matches.Realtime.Types;

namespace Cheetah.Matches.Realtime.DOA.Income.ByField
{
    public class AbstractIncomeByFieldCommandCollector<T> : IDisposable
    {
        internal readonly ReferenceList<Item> stream = new();
        protected readonly ushort fieldId;
        protected readonly CheetahClient client;


        public struct Item
        {
            public ushort commandCreator;
            public CheetahObject cheetahObject;
            public T value;
        }

        protected AbstractIncomeByFieldCommandCollector(CheetahClient client, ushort fieldId)
        {
            this.client = client;
            this.fieldId = fieldId;
            this.client.BeforeUpdateHook += OnBeforeUpdate;
        }

        private void OnBeforeUpdate()
        {
            stream.Clear();
        }

        public ReadonlyReferenceList<Item> GetStream()
        {
            return stream;
        }

        public virtual void Dispose()
        {
            client.BeforeUpdateHook -= OnBeforeUpdate;
        }
    }
}