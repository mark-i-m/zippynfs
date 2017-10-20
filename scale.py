
import matplotlib.pyplot as plt

nclients = [i for i in range(1,10)]
bw = reversed([
 2.1
, 2.2
, 2.5
, 2.7
, 3.1
, 3.5
, 3.8
, 4.6
, 7.2
])

bw = [bw * n for (bw, n) in zip(bw, nclients)]

plt.plot(nclients, bw, marker="o", color="black")

plt.xlabel('Number of clients')
plt.ylabel('Total Bandwidth (MB/s)')

plt.title('Total Bandwidth for 1MiB writes for Multiple Clients')

plt.grid(True)

plt.show()
