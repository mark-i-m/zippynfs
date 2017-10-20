
import numpy as np
import matplotlib.pyplot as plt

# [sync, async]
aws_aws = [218.5, 1.4]
aws_lh  = [259.7, 145.9]
lh_lh   = [446.4, 0.3]

ind = np.arange(2)  # the x locations for the groups
width = 0.20       # the width of the bars

colors=['#DEA4BD', '#B74576','#A6CEE3', '#1F78B4', '#969696', '#252525']

rects1 = plt.bar(ind, aws_aws, width, color=colors[0])
rects2 = plt.bar(ind + width, aws_lh, width, color=colors[1])
rects3 = plt.bar(ind + 2*width, lh_lh, width, color=colors[2])

plt.yscale('log')

plt.yticks([0.1, 1.0, 10, 100, 1000], ['100ms', '1s', '10s', '100s', '1ks'])

plt.ylabel('Latency')

plt.xticks([i + 3*width / 2 for i in [0,1]], ['FILE_SYNC', 'UNSTABLE + Commit'])

plt.title('Latency of 10MiB write with FILE_SYNC vs UNSTABLE Writes')

plt.legend((rects1[0], rects2[0], rects3[0]), ('Client: AWS, Server: AWS', 'Client: seclab8, Server: AWS', 'Client: seclab8, Server: seclab8'))

plt.grid(True)

plt.show()
